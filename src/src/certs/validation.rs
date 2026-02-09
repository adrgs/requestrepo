use anyhow::{anyhow, Context, Result};
use std::net::IpAddr;
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
            });
        }

        // Certificate is valid
        Ok(ValidationResult {
            needs_renewal: false,
            reason: None,
            days_until_expiry: Some(days_until_expiry),
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
                if let GeneralName::DNSName(dns) = name {
                    domains.push(dns.to_string());
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
        Ok(seconds_until_expiry(cert)? / 86400)
    }
}

/// Validates IP certificates for expiry and IP SAN match
/// Uses hours-level precision since IP certs are short-lived (6 days)
pub struct IpCertValidator {
    expected_ip: IpAddr,
    renewal_hours: u64,
}

impl IpCertValidator {
    pub fn new(expected_ip: IpAddr, renewal_hours: u64) -> Self {
        Self {
            expected_ip,
            renewal_hours,
        }
    }

    /// Validate an IP certificate and determine if renewal is needed
    pub fn validate(&self, cert_pem: &[u8]) -> Result<ValidationResult> {
        let pem_data = ::pem::parse(cert_pem).context("Failed to parse IP certificate PEM")?;
        let cert_der = pem_data.contents();
        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| anyhow!("Failed to parse IP X.509 certificate: {:?}", e))?;

        // Check IP SAN match
        if !self.check_ip_match(&cert)? {
            return Ok(ValidationResult {
                needs_renewal: true,
                reason: Some(format!("IP SAN mismatch: expected {}", self.expected_ip)),
                days_until_expiry: None,
            });
        }

        // Check expiry in hours (short-lived certs need finer granularity)
        let hours_until_expiry = seconds_until_expiry(&cert)? / 3600;
        let days_until_expiry = hours_until_expiry / 24;

        info!(
            "IP certificate expires in {} hours ({} days), renewal threshold: {} hours",
            hours_until_expiry, days_until_expiry, self.renewal_hours
        );

        if hours_until_expiry < self.renewal_hours as i64 {
            return Ok(ValidationResult {
                needs_renewal: true,
                reason: Some(format!(
                    "IP certificate expires in {} hours (threshold: {} hours)",
                    hours_until_expiry, self.renewal_hours
                )),
                days_until_expiry: Some(days_until_expiry),
            });
        }

        Ok(ValidationResult {
            needs_renewal: false,
            reason: None,
            days_until_expiry: Some(days_until_expiry),
        })
    }

    /// Check if the certificate contains an IP SAN matching the expected IP
    fn check_ip_match(&self, cert: &X509Certificate) -> Result<bool> {
        let san_ext = match cert.subject_alternative_name() {
            Ok(Some(ext)) => ext,
            _ => return Ok(false),
        };

        for name in &san_ext.value.general_names {
            if let GeneralName::IPAddress(bytes) = name {
                let ip = match bytes.len() {
                    4 => {
                        let octets: [u8; 4] = bytes[..4].try_into().unwrap();
                        IpAddr::from(octets)
                    }
                    16 => {
                        let octets: [u8; 16] = bytes[..16].try_into().unwrap();
                        IpAddr::from(octets)
                    }
                    _ => continue,
                };

                if ip == self.expected_ip {
                    debug!("IP SAN match: {}", ip);
                    return Ok(true);
                }
            }
        }

        warn!("No matching IP SAN found for {}", self.expected_ip);
        Ok(false)
    }
}

/// Calculate seconds until certificate expiry
fn seconds_until_expiry(cert: &X509Certificate) -> Result<i64> {
    let expiry_secs = cert.validity().not_after.timestamp();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time before Unix epoch")?;
    Ok(expiry_secs - now.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed test certificate for testing (expires in 365 days from generation)
    // In real tests, we'd generate fresh certs or use a fixture
    #[allow(dead_code)]
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

    #[test]
    fn test_ip_validator_creation() {
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let validator = IpCertValidator::new(ip, 96);
        assert_eq!(validator.expected_ip, ip);
        assert_eq!(validator.renewal_hours, 96);
    }

    #[test]
    fn test_ip_cert_validation_with_rcgen() {
        use rcgen::{CertificateParams, KeyPair};

        let key_pair = KeyPair::generate().unwrap();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let mut params = CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.subject_alt_names = vec![rcgen::SanType::IpAddress(ip)];
        // Self-signed cert for testing
        let cert = params.self_signed(&key_pair).unwrap();
        let cert_pem = cert.pem();

        // Matching IP
        let validator = IpCertValidator::new(ip, 96);
        let result = validator.validate(cert_pem.as_bytes()).unwrap();
        assert!(!result.needs_renewal);
        assert!(result.days_until_expiry.is_some());

        // Non-matching IP
        let wrong_ip: IpAddr = "10.0.0.2".parse().unwrap();
        let validator2 = IpCertValidator::new(wrong_ip, 96);
        let result2 = validator2.validate(cert_pem.as_bytes()).unwrap();
        assert!(result2.needs_renewal);
        assert!(result2.reason.unwrap().contains("IP SAN mismatch"));
    }
}
