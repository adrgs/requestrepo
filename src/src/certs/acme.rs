use anyhow::{anyhow, Context, Result};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewAccount, NewOrder, RetryPolicy,
};
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::challenge::{DnsChallengeHandler, HttpChallengeHandler};
use super::storage::CertStorage;

/// ACME client for Let's Encrypt certificate issuance
pub struct AcmeClient {
    account: Account,
}

impl AcmeClient {
    /// Create a new ACME client, loading or creating account credentials
    pub async fn new(
        directory_url: &str,
        email: Option<&str>,
        storage: &CertStorage,
    ) -> Result<Self> {
        let account = match storage.load_account()? {
            Some(creds) => {
                info!("Using existing ACME account");
                Account::builder()
                    .context("Failed to create account builder")?
                    .from_credentials(creds)
                    .await
                    .context("Failed to load account from credentials")?
            }
            None => {
                info!("Creating new ACME account");
                let contact: Vec<String> = email
                    .map(|e| vec![format!("mailto:{}", e)])
                    .unwrap_or_default();

                let (account, creds) = Account::builder()
                    .context("Failed to create account builder")?
                    .create(
                        &NewAccount {
                            contact: &contact.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                            terms_of_service_agreed: true,
                            only_return_existing: false,
                        },
                        directory_url.to_owned(),
                        None,
                    )
                    .await
                    .context("Failed to create ACME account")?;

                storage.save_account(&creds)?;
                account
            }
        };

        Ok(Self { account })
    }

    /// Obtain a certificate for the given domain and wildcard via DNS-01
    /// Returns (certificate_chain_pem, private_key_pem)
    pub async fn obtain_certificate(
        &self,
        domain: &str,
        handler: &DnsChallengeHandler,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        info!("Requesting certificate for {} and *.{}", domain, domain);

        let identifiers = vec![
            Identifier::Dns(domain.to_string()),
            Identifier::Dns(format!("*.{domain}")),
        ];

        let mut order = self
            .account
            .new_order(&NewOrder::new(&identifiers))
            .await
            .context("Failed to create ACME order")?;

        // Process DNS-01 authorizations sequentially
        // DnsChallengeHandler::set_txt() appends values, so wildcard certs work correctly
        let mut challenge_domains: HashSet<String> = HashSet::new();

        {
            let mut authorizations = order.authorizations();
            while let Some(result) = authorizations.next().await {
                let mut authz = result.context("Failed to get authorization")?;

                if authz.status == AuthorizationStatus::Valid {
                    debug!("Authorization already valid, skipping");
                    continue;
                }

                let auth_ident = authz.identifier();
                let ident_str = match &auth_ident.identifier {
                    Identifier::Dns(d) => d.clone(),
                    _ => {
                        warn!("Unexpected identifier type in DNS authorization");
                        continue;
                    }
                };

                let mut challenge = authz
                    .challenge(ChallengeType::Dns01)
                    .ok_or_else(|| anyhow!("No DNS-01 challenge found for {}", ident_str))?;

                let key_auth = challenge.key_authorization();
                let txt_value = key_auth.dns_value();
                let challenge_domain =
                    format!("_acme-challenge.{}", ident_str.trim_start_matches("*."));

                info!(
                    "Setting DNS-01 challenge for {}: {} = {}...",
                    ident_str,
                    challenge_domain,
                    &txt_value[..20.min(txt_value.len())]
                );

                handler.set_txt(&challenge_domain, &txt_value).await?;
                handler
                    .wait_propagation(&challenge_domain, &txt_value, 1200)
                    .await?;

                challenge
                    .set_ready()
                    .await
                    .context(format!("Failed to mark challenge ready for {ident_str}"))?;

                challenge_domains.insert(challenge_domain);
            }
        }

        // Wait for order to become ready (all authorizations validated)
        let retry = RetryPolicy::new().timeout(Duration::from_secs(120));
        order
            .poll_ready(&retry)
            .await
            .context("Order did not become ready")?;

        // Clean up TXT records
        for domain in &challenge_domains {
            info!("Clearing TXT record for {}", domain);
            handler.clear_txt(domain).await?;
        }

        // Generate CSR and finalize
        let sans = vec![
            rcgen::SanType::DnsName(domain.try_into().unwrap()),
            rcgen::SanType::DnsName(format!("*.{domain}").try_into().unwrap()),
        ];
        let (key_pair, csr_der) = self.generate_csr(sans)?;
        order
            .finalize_csr(&csr_der)
            .await
            .context("Failed to finalize order")?;

        let cert_chain = order
            .poll_certificate(&retry)
            .await
            .context("Failed to get certificate")?;

        let key_pem = key_pair.serialize_pem().into_bytes();

        info!("Certificate obtained successfully for {}", domain);
        Ok((cert_chain.into_bytes(), key_pem))
    }

    /// Obtain a short-lived certificate for an IP address via HTTP-01
    /// Returns (certificate_chain_pem, private_key_pem)
    pub async fn obtain_ip_certificate(
        &self,
        ip: IpAddr,
        http_handler: &HttpChallengeHandler,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        info!("Requesting short-lived IP certificate for {}", ip);

        let identifiers = vec![Identifier::Ip(ip)];
        let new_order = NewOrder::new(&identifiers).profile("shortlived");

        let mut order = self
            .account
            .new_order(&new_order)
            .await
            .context("Failed to create ACME order for IP")?;

        // Process HTTP-01 authorization
        let mut tokens_to_clear = Vec::new();

        {
            let mut authorizations = order.authorizations();
            while let Some(result) = authorizations.next().await {
                let mut authz = result.context("Failed to get IP authorization")?;

                if authz.status == AuthorizationStatus::Valid {
                    debug!("IP authorization already valid, skipping");
                    continue;
                }

                let mut challenge = authz
                    .challenge(ChallengeType::Http01)
                    .ok_or_else(|| anyhow!("No HTTP-01 challenge for IP {}", ip))?;

                let key_auth = challenge.key_authorization();
                let token = challenge.token.clone();

                http_handler.set_token(&token, key_auth.as_str());
                info!(
                    "HTTP-01 challenge set: serving at /.well-known/acme-challenge/{}",
                    token
                );

                tokens_to_clear.push(token);

                challenge
                    .set_ready()
                    .await
                    .context("Failed to mark HTTP-01 challenge ready")?;
            }
        }

        // Wait for order to become ready
        let retry = RetryPolicy::new().timeout(Duration::from_secs(120));
        order
            .poll_ready(&retry)
            .await
            .context("IP cert order did not become ready")?;

        // Clear tokens after validation
        for token in &tokens_to_clear {
            http_handler.clear_token(token);
        }

        // Generate CSR with IP SAN and finalize
        let sans = vec![rcgen::SanType::IpAddress(ip)];
        let (key_pair, csr_der) = self.generate_csr(sans)?;
        order
            .finalize_csr(&csr_der)
            .await
            .context("Failed to finalize IP cert order")?;

        let cert_chain = order
            .poll_certificate(&retry)
            .await
            .context("Failed to get IP certificate")?;

        let key_pem = key_pair.serialize_pem().into_bytes();

        info!("IP certificate obtained successfully for {}", ip);
        Ok((cert_chain.into_bytes(), key_pem))
    }

    /// Generate a private key and CSR with the given Subject Alternative Names
    fn generate_csr(&self, sans: Vec<rcgen::SanType>) -> Result<(KeyPair, Vec<u8>)> {
        let key_pair = KeyPair::generate().context("Failed to generate key pair")?;

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.subject_alt_names = sans;

        let csr = params
            .serialize_request(&key_pair)
            .context("Failed to serialize CSR")?;

        Ok((key_pair, csr.der().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use rcgen::{CertificateParams, DistinguishedName, KeyPair};

    #[test]
    fn test_csr_generation() {
        let key_pair = KeyPair::generate().expect("Failed to generate key pair");

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName("example.com".try_into().unwrap()),
            rcgen::SanType::DnsName("*.example.com".try_into().unwrap()),
        ];

        let csr = params
            .serialize_request(&key_pair)
            .expect("Failed to serialize CSR");
        let csr_der = csr.der();

        assert!(!csr_der.is_empty());
    }

    #[test]
    fn test_ip_csr_generation() {
        let key_pair = KeyPair::generate().expect("Failed to generate key pair");

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.subject_alt_names = vec![rcgen::SanType::IpAddress("1.2.3.4".parse().unwrap())];

        let csr = params
            .serialize_request(&key_pair)
            .expect("Failed to serialize IP CSR");
        let csr_der = csr.der();

        assert!(!csr_der.is_empty());
    }

    #[test]
    fn test_key_pair_serialization() {
        let key_pair = KeyPair::generate().expect("Failed to generate key pair");
        let pem = key_pair.serialize_pem();

        assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(pem.ends_with("-----END PRIVATE KEY-----\n"));
    }
}
