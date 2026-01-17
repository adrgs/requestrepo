use anyhow::{anyhow, Context, Result};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewAccount, NewOrder, Order,
    OrderStatus,
};
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::challenge::DnsChallengeHandler;
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

        Ok(Self { account })
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
            Identifier::Dns(format!("*.{domain}")),
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

        // For wildcard certs, both authorizations use the same TXT record domain
        // We must set ALL TXT records, validate ALL, then clear ALL
        // to avoid clearing a record that another authorization still needs
        self.process_all_authorizations(&mut order, &authorizations, handler)
            .await?;

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

    /// Process all authorizations together to handle shared TXT record domains
    /// This is required for wildcard certs where base and wildcard use the same _acme-challenge domain
    ///
    /// IMPORTANT: For wildcard certificates (e.g., requestrepo.com + *.requestrepo.com):
    /// - BOTH authorizations have identifier = "requestrepo.com" (per RFC 8555)
    /// - Each authorization has a DIFFERENT challenge token
    /// - BOTH TXT records must be set simultaneously at _acme-challenge.requestrepo.com
    /// - We track by challenge URL (unique) not identifier (ambiguous)
    async fn process_all_authorizations(
        &self,
        order: &mut Order,
        authorizations: &[instant_acme::Authorization],
        handler: &DnsChallengeHandler,
    ) -> Result<()> {
        use std::collections::HashMap;

        // Collect ALL challenges from ALL authorizations
        // Key: challenge_url (unique), Value: (identifier, challenge_domain, txt_value, needs_validation)
        let mut challenges: HashMap<String, (String, String, String, bool)> = HashMap::new();

        // Also track which domains we set TXT records for (for cleanup)
        let mut challenge_domains: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for auth in authorizations {
            let identifier = match &auth.identifier {
                Identifier::Dns(domain) => domain.clone(),
            };

            let needs_validation = !matches!(auth.status, AuthorizationStatus::Valid);

            info!(
                "Processing authorization for {} (status={:?}, needs_validation={})",
                identifier, auth.status, needs_validation
            );

            // Find the DNS-01 challenge
            let dns_challenge = match auth
                .challenges
                .iter()
                .find(|c| c.r#type == ChallengeType::Dns01)
            {
                Some(c) => c,
                None => {
                    if needs_validation {
                        return Err(anyhow!("No DNS-01 challenge found for {}", identifier));
                    }
                    // Already valid, no challenge needed
                    continue;
                }
            };

            // Get the challenge token
            let key_auth = order.key_authorization(dns_challenge);
            let txt_value = key_auth.dns_value();

            // For wildcards, use _acme-challenge.{base_domain}
            let challenge_domain =
                format!("_acme-challenge.{}", identifier.trim_start_matches("*."));

            info!(
                "  Challenge URL: {}, domain: {}, token: {}...",
                dns_challenge.url,
                challenge_domain,
                &txt_value[..20.min(txt_value.len())]
            );

            challenges.insert(
                dns_challenge.url.clone(),
                (
                    identifier,
                    challenge_domain.clone(),
                    txt_value,
                    needs_validation,
                ),
            );
            challenge_domains.insert(challenge_domain);
        }

        // Count how many challenges need validation
        let pending_count = challenges.values().filter(|(_, _, _, n)| *n).count();

        if pending_count == 0 {
            debug!("All authorizations already valid, no challenges to process");
            return Ok(());
        }

        info!(
            "Setting up {} TXT records for {} pending challenges",
            challenge_domains.len(),
            pending_count
        );

        // Collect all TXT values per challenge domain
        let mut txt_by_domain: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for (url, (identifier, challenge_domain, txt_value, needs_validation)) in &challenges {
            if *needs_validation {
                txt_by_domain
                    .entry(challenge_domain.clone())
                    .or_default()
                    .push((identifier.clone(), txt_value.clone()));
                info!(
                    "Will set TXT for {} at {} (challenge URL: {})",
                    identifier,
                    challenge_domain,
                    &url[url.len().saturating_sub(30)..]
                );
            }
        }

        // Set ALL TXT records before notifying ACME
        for (challenge_domain, txt_entries) in &txt_by_domain {
            info!(
                "Setting {} TXT record(s) for domain {}",
                txt_entries.len(),
                challenge_domain
            );

            for (identifier, txt_value) in txt_entries {
                info!(
                    "  Setting TXT for {}: {} = {}...",
                    identifier,
                    challenge_domain,
                    &txt_value[..20.min(txt_value.len())]
                );
                handler.set_txt(challenge_domain, txt_value).await?;
            }

            // Wait for propagation - check for ALL TXT values we set
            // (DNS can return multiple TXT records for the same name)
            for (identifier, txt_value) in txt_entries {
                info!(
                    "Waiting for TXT propagation for {}: {}",
                    identifier, challenge_domain
                );
                handler
                    .wait_propagation(challenge_domain, txt_value, 1200)
                    .await?;
            }
        }

        // Notify ACME server about ALL pending challenges being ready
        info!("Notifying ACME server about all challenges being ready");
        for (url, (identifier, _, _, needs_validation)) in &challenges {
            if *needs_validation {
                info!(
                    "  Notifying ACME for {} (url: ...{})",
                    identifier,
                    &url[url.len().saturating_sub(30)..]
                );
                order
                    .set_challenge_ready(url)
                    .await
                    .context(format!("Failed to notify ACME server for {identifier}"))?;
            }
        }

        // Wait for all authorizations to be validated by polling order state
        // This is more robust than waiting for individual authorizations by identifier
        // (which is ambiguous for wildcard certs)
        info!("Waiting for all authorizations to validate...");
        self.wait_for_all_authorizations_valid(order, &challenges)
            .await?;

        // Clean up all TXT records after all validations complete
        for challenge_domain in &challenge_domains {
            info!("Clearing TXT record for {}", challenge_domain);
            handler.clear_txt(challenge_domain).await?;
        }

        Ok(())
    }

    /// Wait for all authorizations to become valid by polling order state
    /// This avoids the identifier ambiguity issue with wildcard certs
    async fn wait_for_all_authorizations_valid(
        &self,
        order: &mut Order,
        challenges: &std::collections::HashMap<String, (String, String, String, bool)>,
    ) -> Result<()> {
        let max_attempts = 60; // 2 minutes at 2s intervals
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > max_attempts {
                return Err(anyhow!("Authorization validation timeout"));
            }

            // Fetch fresh authorizations
            let fresh_auths = order
                .authorizations()
                .await
                .context("Failed to refresh authorizations")?;

            // Check status of each authorization by matching challenge URLs
            let mut all_valid = true;
            let mut any_invalid = false;

            for fresh_auth in &fresh_auths {
                let identifier = match &fresh_auth.identifier {
                    Identifier::Dns(d) => d.clone(),
                };

                // Find matching challenge URL
                let matching_challenge_url = fresh_auth
                    .challenges
                    .iter()
                    .find(|c| c.r#type == ChallengeType::Dns01)
                    .map(|c| c.url.clone());

                let needs_validation = matching_challenge_url
                    .as_ref()
                    .and_then(|url| challenges.get(url))
                    .map(|(_, _, _, n)| *n)
                    .unwrap_or(false);

                if !needs_validation {
                    // This authorization was already valid, skip
                    continue;
                }

                match fresh_auth.status {
                    AuthorizationStatus::Valid => {
                        debug!("Authorization validated for {}", identifier);
                    }
                    AuthorizationStatus::Invalid => {
                        // Log challenge errors for debugging
                        for challenge in &fresh_auth.challenges {
                            if let Some(ref error) = challenge.error {
                                info!(
                                    "Challenge error for {} (type={:?}): {:?}",
                                    identifier, challenge.r#type, error
                                );
                            }
                        }
                        any_invalid = true;
                    }
                    AuthorizationStatus::Pending => {
                        debug!("Authorization still pending for {}", identifier);
                        all_valid = false;
                    }
                    _ => {
                        all_valid = false;
                    }
                }
            }

            if any_invalid {
                return Err(anyhow!(
                    "One or more authorizations became invalid. Check logs for challenge errors."
                ));
            }

            if all_valid {
                info!("All authorizations validated successfully");
                return Ok(());
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
            rcgen::SanType::DnsName(format!("*.{domain}").try_into().unwrap()),
        ];

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
        // Test CSR generation directly without AcmeClient
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
