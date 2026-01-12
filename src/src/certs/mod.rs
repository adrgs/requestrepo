mod acme;
mod challenge;
mod storage;
mod tls;
mod validation;

pub use acme::{ACME_DIRECTORY_PRODUCTION, ACME_DIRECTORY_STAGING};
pub use tls::TlsManager;

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::cache::Cache;
use crate::utils::config::CONFIG;

use acme::AcmeClient;
use challenge::DnsChallengeHandler;
use storage::CertStorage;
use validation::CertValidator;

/// Certificate manager that handles automatic certificate renewal
pub struct CertManager {
    storage: CertStorage,
    validator: CertValidator,
    tls: TlsManager,
    cache: Arc<Cache>,
    domain: String,
    acme_directory: String,
    acme_email: Option<String>,
    check_interval_hours: u64,
}

impl CertManager {
    /// Create a new CertManager
    /// Loads existing certificates if available
    pub async fn new(cache: Arc<Cache>) -> Result<Self> {
        let domain = CONFIG.server_domain.clone();
        let cert_dir = CONFIG.cert_dir.clone();
        let renewal_days = CONFIG.cert_renewal_days;
        let acme_directory = CONFIG.acme_directory.clone();
        let acme_email = CONFIG.acme_email.clone();
        let check_interval_hours = CONFIG.cert_check_hours;

        info!("Initializing CertManager for domain: {}", domain);
        info!("Certificate directory: {}", cert_dir);
        info!("ACME directory: {}", acme_directory);
        info!("Renewal threshold: {} days", renewal_days);

        let storage = CertStorage::new(&cert_dir)?;
        let validator = CertValidator::new(domain.clone(), renewal_days);
        let tls = TlsManager::new();

        let manager = Self {
            storage,
            validator,
            tls,
            cache,
            domain,
            acme_directory,
            acme_email,
            check_interval_hours,
        };

        // Try to load existing certificate
        if let Err(e) = manager.load_existing_cert() {
            warn!("No existing certificate loaded: {}", e);
        }

        Ok(manager)
    }

    /// Get the TLS manager for use by the HTTPS server
    pub fn tls_manager(&self) -> TlsManager {
        self.tls.clone()
    }

    /// Check if HTTPS is ready (certificate loaded)
    pub fn is_ready(&self) -> bool {
        self.tls.is_configured()
    }

    /// Load existing certificate from storage
    fn load_existing_cert(&self) -> Result<()> {
        if let Some((chain, key)) = self.storage.load_certificate()? {
            // Validate the certificate before loading
            let result = self.validator.validate(&chain)?;

            if result.needs_renewal {
                warn!(
                    "Existing certificate needs renewal: {}",
                    result.reason.unwrap_or_default()
                );
                // Still load it for now, renewal task will get a new one
            } else {
                info!(
                    "Existing certificate is valid (expires in {} days)",
                    result.days_until_expiry.unwrap_or(0)
                );
            }

            // Load into TLS manager
            self.tls.reload(&chain, &key)?;
            info!("Loaded existing certificate for {}", self.domain);
        }

        Ok(())
    }

    /// Start the background renewal task
    pub fn start_renewal_task(self: Arc<Self>) {
        let check_hours = self.check_interval_hours;

        tokio::spawn(async move {
            // Initial check after 10 seconds
            sleep(Duration::from_secs(10)).await;

            loop {
                if let Err(e) = self.check_and_renew().await {
                    error!("Certificate renewal check failed: {}", e);
                }

                // Wait before next check
                let interval = Duration::from_secs(check_hours * 3600);
                info!("Next certificate check in {} hours", check_hours);
                sleep(interval).await;
            }
        });
    }

    /// Check if renewal is needed and perform it
    /// This can be called manually to trigger immediate renewal check
    pub async fn check_and_renew(&self) -> Result<()> {
        info!("Checking certificate status for {}", self.domain);

        // Load current certificate
        let needs_renewal = match self.storage.load_certificate()? {
            Some((chain, _key)) => {
                let result = self.validator.validate(&chain)?;

                if result.needs_renewal {
                    info!(
                        "Certificate needs renewal: {}",
                        result.reason.unwrap_or_default()
                    );
                    true
                } else {
                    info!(
                        "Certificate is valid for {} more days",
                        result.days_until_expiry.unwrap_or(0)
                    );
                    false
                }
            }
            None => {
                info!("No certificate found, will request new one");
                true
            }
        };

        if needs_renewal {
            self.obtain_certificate().await?;
        }

        Ok(())
    }

    /// Obtain a new certificate from Let's Encrypt
    async fn obtain_certificate(&self) -> Result<()> {
        info!("Requesting new certificate for {}", self.domain);

        // Initialize ACME client
        let acme = AcmeClient::new(
            &self.acme_directory,
            self.acme_email.as_deref(),
            &self.storage,
        )
        .await
        .context("Failed to initialize ACME client")?;

        // Create challenge handler
        let handler = DnsChallengeHandler::new(Arc::clone(&self.cache));

        // Attempt certificate issuance with retries
        let mut last_error = None;
        for attempt in 1..=3 {
            info!("Certificate request attempt {}/3", attempt);

            match acme.obtain_certificate(&self.domain, &handler).await {
                Ok((chain, key)) => {
                    // Save to storage (atomic write)
                    self.storage.save_certificate(&chain, &key)?;

                    // Hot-reload TLS config
                    self.tls.reload(&chain, &key)?;

                    info!(
                        "Successfully obtained and loaded certificate for {}",
                        self.domain
                    );
                    return Ok(());
                }
                Err(e) => {
                    error!("Certificate request attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < 3 {
                        let delay = Duration::from_secs(60 * (1 << (attempt - 1))); // 1min, 2min
                        info!("Retrying in {:?}", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_fields() {
        // Just verify the config fields exist and have expected types
        let _ = &CONFIG.tls_enabled;
        let _ = &CONFIG.https_port;
        let _ = &CONFIG.cert_dir;
        let _ = &CONFIG.acme_email;
        let _ = &CONFIG.acme_directory;
        let _ = &CONFIG.cert_renewal_days;
        let _ = &CONFIG.cert_check_hours;
    }
}
