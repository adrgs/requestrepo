use anyhow::{anyhow, Context, Result};
use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, NewAccount,
    NewOrder, Order, OrderStatus,
};
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use super::challenge::DnsChallengeHandler;
use super::storage::CertStorage;

/// Let's Encrypt production ACME directory URL
pub const ACME_DIRECTORY_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// Let's Encrypt staging ACME directory URL (use for testing!)
/// Staging has much higher rate limits and issues test certificates
pub const ACME_DIRECTORY_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

/// ACME client for Let's Encrypt certificate issuance
pub struct AcmeClient {
    account: Account,
    directory_url: String,
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
                Account::from_credentials(creds)
                    .await
                    .context("Failed to load account from credentials")?
            }
            None => {
                info!("Creating new ACME account");
                let contact: Vec<String> = email
                    .map(|e| vec![format!("mailto:{}", e)])
                    .unwrap_or_default();

                let (account, creds) = Account::create(
                    &NewAccount {
                        contact: &contact.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        terms_of_service_agreed: true,
                        only_return_existing: false,
                    },
                    directory_url,
                    None,
                )
                .await
                .context("Failed to create ACME account")?;

                storage.save_account(&creds)?;
                account
            }
        };

        Ok(Self {
            account,
            directory_url: directory_url.to_string(),
        })
    }

    /// Obtain a certificate for the given domain and wildcard
    /// Returns (certificate_chain_pem, private_key_pem)
    pub async fn obtain_certificate(
        &self,
        domain: &str,
        handler: &DnsChallengeHandler,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        info!("Requesting certificate for {} and *.{}", domain, domain);

        // Create order for both the base domain and wildcard
        let identifiers = vec![
            Identifier::Dns(domain.to_string()),
            Identifier::Dns(format!("*.{}", domain)),
        ];

        let mut order = self
            .account
            .new_order(&NewOrder {
                identifiers: &identifiers,
            })
            .await
            .context("Failed to create ACME order")?;

        // Get authorizations and complete DNS challenges
        let authorizations = order
            .authorizations()
            .await
            .context("Failed to get authorizations")?;

        info!("Processing {} authorizations", authorizations.len());

        for auth in &authorizations {
            self.process_authorization(&mut order, auth, handler).await?;
        }

        // Wait for order to be ready
        self.wait_for_order_ready(&mut order).await?;

        // Generate private key and CSR
        let (key_pair, csr_der) = self.generate_csr(domain)?;

        // Finalize the order
        order
            .finalize(&csr_der)
            .await
            .context("Failed to finalize order")?;

        // Wait for certificate
        let cert_chain = self.wait_for_certificate(&mut order).await?;

        // Serialize the private key to PEM
        let key_pem = key_pair.serialize_pem().into_bytes();

        info!("Certificate obtained successfully for {}", domain);
        Ok((cert_chain.into_bytes(), key_pem))
    }

    /// Process a single authorization by completing its DNS-01 challenge
    async fn process_authorization(
        &self,
        order: &mut Order,
        auth: &instant_acme::Authorization,
        handler: &DnsChallengeHandler,
    ) -> Result<()> {
        let identifier = match &auth.identifier {
            Identifier::Dns(domain) => domain.clone(),
        };

        info!("Processing authorization for {}", identifier);

        // Skip if already valid
        if matches!(auth.status, AuthorizationStatus::Valid) {
            debug!("Authorization already valid for {}", identifier);
            return Ok(());
        }

        // Find the DNS-01 challenge
        let dns_challenge = auth
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Dns01)
            .ok_or_else(|| anyhow!("No DNS-01 challenge found for {}", identifier))?;

        // Get the challenge token (key authorization digest)
        let key_auth = order.key_authorization(dns_challenge);
        let txt_value = key_auth.dns_value();

        // The challenge domain is _acme-challenge.{domain}
        // For wildcards, use _acme-challenge.{base_domain}
        let challenge_domain = format!("_acme-challenge.{}", identifier.trim_start_matches("*."));

        // Set the TXT record
        handler.set_txt(&challenge_domain, &txt_value).await?;

        // Wait for DNS propagation (20 minute timeout)
        handler
            .wait_propagation(&challenge_domain, &txt_value, 1200)
            .await?;

        // Tell ACME server we're ready
        order
            .set_challenge_ready(&dns_challenge.url)
            .await
            .context("Failed to notify ACME server of challenge completion")?;

        // Wait for challenge validation
        self.wait_for_challenge_valid(order, auth).await?;

        // Clean up TXT record
        handler.clear_txt(&challenge_domain).await?;

        Ok(())
    }

    /// Wait for a challenge to be validated
    async fn wait_for_challenge_valid(
        &self,
        order: &mut Order,
        auth: &instant_acme::Authorization,
    ) -> Result<()> {
        let identifier = match &auth.identifier {
            Identifier::Dns(domain) => domain.clone(),
        };

        let max_attempts = 30;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                return Err(anyhow!(
                    "Challenge validation timeout for {}",
                    identifier
                ));
            }

            // Fetch fresh authorization state
            let fresh_auths = order
                .authorizations()
                .await
                .context("Failed to refresh authorizations")?;

            // Find our authorization
            if let Some(fresh_auth) = fresh_auths.iter().find(|a| match &a.identifier {
                Identifier::Dns(d) => d == &identifier,
            }) {
                match fresh_auth.status {
                    AuthorizationStatus::Valid => {
                        info!("Authorization validated for {}", identifier);
                        return Ok(());
                    }
                    AuthorizationStatus::Invalid => {
                        return Err(anyhow!("Authorization invalid for {}", identifier));
                    }
                    AuthorizationStatus::Pending => {
                        debug!("Authorization still pending for {}", identifier);
                    }
                    _ => {}
                }
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    /// Wait for order to be ready for finalization
    async fn wait_for_order_ready(&self, order: &mut Order) -> Result<()> {
        let max_attempts = 30;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                return Err(anyhow!("Order ready timeout"));
            }

            order.refresh().await.context("Failed to refresh order")?;

            match order.state().status {
                OrderStatus::Ready => {
                    info!("Order is ready for finalization");
                    return Ok(());
                }
                OrderStatus::Invalid => {
                    return Err(anyhow!("Order became invalid"));
                }
                OrderStatus::Pending => {
                    debug!("Order still pending");
                }
                _ => {}
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    /// Wait for certificate to be issued
    async fn wait_for_certificate(&self, order: &mut Order) -> Result<String> {
        let max_attempts = 30;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                return Err(anyhow!("Certificate issuance timeout"));
            }

            order.refresh().await.context("Failed to refresh order")?;

            match order.state().status {
                OrderStatus::Valid => {
                    let cert = order
                        .certificate()
                        .await
                        .context("Failed to download certificate")?
                        .ok_or_else(|| anyhow!("No certificate returned"))?;

                    info!("Certificate downloaded successfully");
                    return Ok(cert);
                }
                OrderStatus::Invalid => {
                    return Err(anyhow!("Order became invalid"));
                }
                OrderStatus::Processing => {
                    debug!("Certificate still processing");
                }
                _ => {}
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    /// Generate a private key and CSR for the given domain
    fn generate_csr(&self, domain: &str) -> Result<(KeyPair, Vec<u8>)> {
        let key_pair = KeyPair::generate().context("Failed to generate key pair")?;

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();

        // Add both the base domain and wildcard as SANs
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(domain.try_into().unwrap()),
            rcgen::SanType::DnsName(format!("*.{}", domain).try_into().unwrap()),
        ];

        let csr = params
            .serialize_request(&key_pair)
            .context("Failed to serialize CSR")?;

        Ok((key_pair, csr.der().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, DistinguishedName, KeyPair};

    #[test]
    fn test_csr_generation() {
        // Test CSR generation directly without AcmeClient
        let key_pair = KeyPair::generate().expect("Failed to generate key pair");

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName("example.com".try_into().unwrap()),
            rcgen::SanType::DnsName("*.example.com".try_into().unwrap()),
        ];

        let csr = params.serialize_request(&key_pair).expect("Failed to serialize CSR");
        let csr_der = csr.der();

        // Verify CSR was generated successfully
        assert!(!csr_der.is_empty());
    }

    #[test]
    fn test_key_pair_serialization() {
        let key_pair = KeyPair::generate().expect("Failed to generate key pair");
        let pem = key_pair.serialize_pem();

        // Verify key is in PEM format
        assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(pem.ends_with("-----END PRIVATE KEY-----\n"));
    }
}
