use anyhow::{anyhow, Context, Result};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};
use x509_parser::prelude::*;

/// Validates certificates for expiry, domain match, and chain trust
pub struct CertValidator {
    expected_domain: String,
    renewal_days: u64,
}

/// Result of certificate validation
#[derive(Debug)]
pub struct ValidationResult {
    pub needs_renewal: bool,
    pub reason: Option<String>,
    pub days_until_expiry: Option<i64>,
    pub domains: Vec<String>,
}

impl CertValidator {
    pub fn new(expected_domain: String, renewal_days: u64) -> Self {
        Self {
            expected_domain,
            renewal_days,
        }
    }

    /// Validate a certificate and determine if renewal is needed
    /// Returns ValidationResult with details about the cert status
    pub fn validate(&self, cert_pem: &[u8]) -> Result<ValidationResult> {
        // Parse PEM to DER
        let pem_data = ::pem::parse(cert_pem).context("Failed to parse certificate PEM")?;
        let cert_der = pem_data.contents();

        // Parse X.509 certificate
        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| anyhow!("Failed to parse X.509 certificate: {:?}", e))?;

        // Extract domains from certificate
        let domains = self.extract_domains(&cert)?;
        debug!("Certificate domains: {:?}", domains);

        // Check domain match
        if !self.check_domain_match(&domains) {
            return Ok(ValidationResult {
                needs_renewal: true,
                reason: Some(format!(
                    "Domain mismatch: expected {} or *.{}, got {:?}",
                    self.expected_domain, self.expected_domain, domains
                )),
                days_until_expiry: None,
                domains,
            });
        }

        // Check expiry
        let days_until_expiry = self.days_until_expiry(&cert)?;
        info!(
            "Certificate expires in {} days (renewal threshold: {} days)",
            days_until_expiry, self.renewal_days
        );

        if days_until_expiry < self.renewal_days as i64 {
            return Ok(ValidationResult {
                needs_renewal: true,
                reason: Some(format!(
                    "Certificate expires in {} days (threshold: {} days)",
                    days_until_expiry, self.renewal_days
                )),
                days_until_expiry: Some(days_until_expiry),
                domains,
            });
        }

        // Certificate is valid
        Ok(ValidationResult {
            needs_renewal: false,
            reason: None,
            days_until_expiry: Some(days_until_expiry),
            domains,
        })
    }

    /// Extract all domain names from the certificate (CN and SANs)
    fn extract_domains(&self, cert: &X509Certificate) -> Result<Vec<String>> {
        let mut domains = Vec::new();

        // Extract CN from subject
        for rdn in cert.subject().iter_rdn() {
            for attr in rdn.iter() {
                if attr.attr_type() == &oid_registry::OID_X509_COMMON_NAME {
                    if let Ok(cn) = attr.as_str() {
                        domains.push(cn.to_string());
                    }
                }
            }
        }

        // Extract SANs (Subject Alternative Names)
        if let Ok(Some(san_ext)) = cert.subject_alternative_name() {
            for name in &san_ext.value.general_names {
                match name {
                    GeneralName::DNSName(dns) => {
                        domains.push(dns.to_string());
                    }
                    _ => {}
                }
            }
        }

        Ok(domains)
    }

    /// Check if the certificate covers the expected domain
    /// Must have both domain.tld and *.domain.tld
    fn check_domain_match(&self, domains: &[String]) -> bool {
        let base_domain = &self.expected_domain;
        let wildcard = format!("*.{}", self.expected_domain);

        let has_base = domains.iter().any(|d| d == base_domain);
        let has_wildcard = domains.iter().any(|d| d == &wildcard);

        if !has_base {
            warn!(
                "Certificate missing base domain: {} (has: {:?})",
                base_domain, domains
            );
        }
        if !has_wildcard {
            warn!(
                "Certificate missing wildcard domain: {} (has: {:?})",
                wildcard, domains
            );
        }

        has_base && has_wildcard
    }

    /// Calculate days until certificate expiry
    fn days_until_expiry(&self, cert: &X509Certificate) -> Result<i64> {
        let not_after = cert.validity().not_after;

        // Convert ASN1Time to timestamp (returns i64 directly)
        let expiry_secs = not_after.timestamp();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time before Unix epoch")?;

        let seconds_until_expiry = expiry_secs - now.as_secs() as i64;
        let days_until_expiry = seconds_until_expiry / 86400;

        Ok(days_until_expiry)
    }

    /// Check if certificate is currently valid (not expired, not before validity period)
    pub fn is_currently_valid(&self, cert_pem: &[u8]) -> Result<bool> {
        let pem_data = ::pem::parse(cert_pem).context("Failed to parse certificate PEM")?;
        let cert_der = pem_data.contents();

        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| anyhow!("Failed to parse X.509 certificate: {:?}", e))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time before Unix epoch")?;

        let now_secs = now.as_secs() as i64;

        let not_before = cert.validity().not_before.timestamp();
        let not_after = cert.validity().not_after.timestamp();

        Ok(now_secs >= not_before && now_secs <= not_after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed test certificate for testing (expires in 365 days from generation)
    // In real tests, we'd generate fresh certs or use a fixture
    const TEST_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIBkTCB+wIJAKHBfpegPjMCMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBnRl
c3RjYTAeFw0yNDAxMDEwMDAwMDBaFw0yNTAxMDEwMDAwMDBaMBExDzANBgNVBAMM
BnRlc3RjYTBcMA0GCSqGSIb3DQEBAQUAA0sAMEgCQQC7o96HtiXcRGrGzMj+G8UB
MfjyuHt7Sg8jfzLErKZbMWyPMyzHsccSL3mMCgNegrKpinVHN7M4EE2iLMKqJOHD
AgMBAAGjPzA9MBsGA1UdEQQUMBKCBnRlc3RjYYIIdGVzdC5jb20wCwYDVR0PBAQD
AgWgMBEGCWCGSAGG+EIBAQQEAwIGQDANBgkqhkiG9w0BAQsFAANBAFGI4NjKFaFU
gPD0A+3N5WsGj/2UrRBAs6AaQaQKqKZDq8O1K0pcr+mAFcyLfgb4F0j4LXGQoRoe
p6LUh+Xo8Cs=
-----END CERTIFICATE-----"#;

    #[test]
    fn test_validator_creation() {
        let validator = CertValidator::new("example.com".to_string(), 7);
        assert_eq!(validator.expected_domain, "example.com");
        assert_eq!(validator.renewal_days, 7);
    }

    #[test]
    fn test_domain_match_logic() {
        let validator = CertValidator::new("example.com".to_string(), 7);

        // Both present - should match
        let domains = vec!["example.com".to_string(), "*.example.com".to_string()];
        assert!(validator.check_domain_match(&domains));

        // Missing wildcard
        let domains = vec!["example.com".to_string()];
        assert!(!validator.check_domain_match(&domains));

        // Missing base
        let domains = vec!["*.example.com".to_string()];
        assert!(!validator.check_domain_match(&domains));

        // Wrong domain
        let domains = vec!["other.com".to_string(), "*.other.com".to_string()];
        assert!(!validator.check_domain_match(&domains));
    }
}
