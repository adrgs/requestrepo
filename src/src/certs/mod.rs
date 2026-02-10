mod acme;
mod challenge;
mod storage;
mod tls;
mod validation;

pub use challenge::HttpChallengeHandler;
pub use tls::TlsManager;

use anyhow::{Context, Result};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::cache::Cache;
use crate::utils::config::CONFIG;

use acme::AcmeClient;
use challenge::DnsChallengeHandler;
use storage::CertStorage;
use validation::{CertValidator, IpCertValidator, ValidationResult};

/// Certificate manager that handles automatic certificate renewal
/// for both domain certs (DNS-01) and IP certs (HTTP-01)
pub struct CertManager {
    storage: CertStorage,
    validator: CertValidator,
    ip_validator: Option<IpCertValidator>,
    tls: TlsManager,
    cache: Arc<Cache>,
    http_challenge_handler: HttpChallengeHandler,
    domain: String,
    server_ip: Option<IpAddr>,
    acme_directory: String,
    acme_email: Option<String>,
    domain_check_interval_hours: u64,
    ip_check_interval_hours: u64,
}

impl CertManager {
    /// Create a new CertManager
    /// Loads existing certificates if available
    pub async fn new(
        cache: Arc<Cache>,
        http_challenge_handler: HttpChallengeHandler,
    ) -> Result<Self> {
        let domain = CONFIG.server_domain.clone();
        let cert_dir = CONFIG.cert_dir.clone();
        let renewal_days = CONFIG.cert_renewal_days;
        let acme_directory = CONFIG.acme_directory.clone();
        let acme_email = CONFIG.acme_email.clone();
        let domain_check_interval_hours = CONFIG.cert_check_hours;
        let ip_check_interval_hours = CONFIG.ip_cert_check_hours;

        info!("Initializing CertManager for domain: {}", domain);
        info!("Certificate directory: {}", cert_dir);
        info!("ACME directory: {}", acme_directory);
        info!("Renewal threshold: {} days", renewal_days);

        let storage = CertStorage::new(&cert_dir)?;
        let validator = CertValidator::new(domain.clone(), renewal_days);
        let tls = TlsManager::new();

        // Parse server IP for IP cert support
        let server_ip: Option<IpAddr> = CONFIG.server_ip.parse().ok();
        let ip_validator = if CONFIG.ip_cert_enabled {
            if let Some(ip) = server_ip {
                info!(
                    "IP certificate support enabled for {} (check every {}h, renew at <{}h)",
                    ip, ip_check_interval_hours, CONFIG.ip_cert_renewal_hours
                );
                Some(IpCertValidator::new(ip, CONFIG.ip_cert_renewal_hours))
            } else {
                warn!(
                    "IP certificate disabled: SERVER_IP '{}' is not a valid IP address",
                    CONFIG.server_ip
                );
                None
            }
        } else {
            info!("IP certificate support disabled");
            None
        };

        let manager = Self {
            storage,
            validator,
            ip_validator,
            tls,
            cache,
            http_challenge_handler,
            domain,
            server_ip,
            acme_directory,
            acme_email,
            domain_check_interval_hours,
            ip_check_interval_hours,
        };

        // Try to load existing certificates
        if let Err(e) = manager.load_existing_domain_cert() {
            warn!("No existing domain certificate loaded: {}", e);
        }
        if let Err(e) = manager.load_existing_ip_cert() {
            warn!("No existing IP certificate loaded: {}", e);
        }

        Ok(manager)
    }

    /// Get the TLS manager for use by the HTTPS server
    pub fn tls_manager(&self) -> TlsManager {
        self.tls.clone()
    }

    /// Load existing domain certificate from storage
    fn load_existing_domain_cert(&self) -> Result<()> {
        if let Some((chain, key)) = self.storage.load_certificate()? {
            let result = self.validator.validate(&chain)?;
            log_validation_result("domain", &result);
            self.tls.reload_domain(&chain, &key)?;
            info!("Loaded existing domain certificate for {}", self.domain);
        }

        Ok(())
    }

    /// Load existing IP certificate from storage
    fn load_existing_ip_cert(&self) -> Result<()> {
        let ip_validator = match &self.ip_validator {
            Some(v) => v,
            None => return Ok(()),
        };

        if let Some((chain, key)) = self.storage.load_ip_certificate()? {
            let result = ip_validator.validate(&chain)?;
            log_validation_result("IP", &result);
            self.tls.reload_ip(&chain, &key)?;
            info!("Loaded existing IP certificate for {:?}", self.server_ip);
        }

        Ok(())
    }

    /// Start the background renewal tasks (domain + IP)
    pub fn start_renewal_task(self: Arc<Self>) {
        // Domain cert renewal task
        let domain_self = Arc::clone(&self);
        let domain_check_hours = self.domain_check_interval_hours;
        tokio::spawn(async move {
            // Initial check after 10 seconds
            sleep(Duration::from_secs(10)).await;

            loop {
                if let Err(e) = domain_self.check_and_renew_domain().await {
                    error!("Domain certificate renewal check failed: {}", e);
                }

                let interval = Duration::from_secs(domain_check_hours * 3600);
                info!(
                    "Next domain certificate check in {} hours",
                    domain_check_hours
                );
                sleep(interval).await;
            }
        });

        // IP cert renewal task (if enabled)
        if self.ip_validator.is_some() {
            let ip_self = Arc::clone(&self);
            let ip_check_hours = self.ip_check_interval_hours;
            tokio::spawn(async move {
                // Staggered start: 30s after domain task
                sleep(Duration::from_secs(30)).await;

                loop {
                    if let Err(e) = ip_self.check_and_renew_ip().await {
                        error!("IP certificate renewal check failed: {}", e);
                    }

                    let interval = Duration::from_secs(ip_check_hours * 3600);
                    info!("Next IP certificate check in {} hours", ip_check_hours);
                    sleep(interval).await;
                }
            });
        }
    }

    /// Check if domain cert renewal is needed and perform it
    pub async fn check_and_renew_domain(&self) -> Result<()> {
        info!("Checking domain certificate status for {}", self.domain);

        let needs_renewal = match self.storage.load_certificate()? {
            Some((chain, _key)) => {
                let result = self.validator.validate(&chain)?;
                log_validation_result("domain", &result);
                result.needs_renewal
            }
            None => {
                info!("No domain certificate found, will request new one");
                true
            }
        };

        if needs_renewal {
            self.obtain_domain_certificate().await?;
        }

        Ok(())
    }

    /// Check if IP cert renewal is needed and perform it
    async fn check_and_renew_ip(&self) -> Result<()> {
        let ip_validator = match &self.ip_validator {
            Some(v) => v,
            None => return Ok(()),
        };

        info!("Checking IP certificate status for {:?}", self.server_ip);

        let needs_renewal = match self.storage.load_ip_certificate()? {
            Some((chain, _key)) => {
                let result = ip_validator.validate(&chain)?;
                log_validation_result("IP", &result);
                result.needs_renewal
            }
            None => {
                info!("No IP certificate found, will request new one");
                true
            }
        };

        if needs_renewal {
            self.obtain_ip_certificate().await?;
        }

        Ok(())
    }

    /// Create a new ACME client using the stored configuration
    async fn create_acme_client(&self) -> Result<AcmeClient> {
        AcmeClient::new(
            &self.acme_directory,
            self.acme_email.as_deref(),
            &self.storage,
        )
        .await
        .context("Failed to initialize ACME client")
    }

    /// Obtain a new domain certificate from Let's Encrypt via DNS-01
    async fn obtain_domain_certificate(&self) -> Result<()> {
        info!("Requesting new domain certificate for {}", self.domain);

        let acme = self.create_acme_client().await?;
        let handler = DnsChallengeHandler::new(Arc::clone(&self.cache));

        let mut last_error = None;
        for attempt in 1..=3 {
            info!("Domain certificate request attempt {}/3", attempt);

            match acme.obtain_certificate(&self.domain, &handler).await {
                Ok((chain, key)) => {
                    self.storage.save_certificate(&chain, &key)?;
                    self.tls.reload_domain(&chain, &key)?;

                    info!(
                        "Successfully obtained and loaded domain certificate for {}",
                        self.domain
                    );
                    return Ok(());
                }
                Err(e) => {
                    error!(
                        "Domain certificate request attempt {} failed: {}",
                        attempt, e
                    );
                    last_error = Some(e);

                    if attempt < 3 {
                        let delay = Duration::from_secs(60 * (1 << (attempt - 1)));
                        info!("Retrying in {:?}", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }

    /// Obtain a new IP certificate from Let's Encrypt via HTTP-01
    async fn obtain_ip_certificate(&self) -> Result<()> {
        let ip = self
            .server_ip
            .expect("IP validator exists, so server_ip must be Some");
        info!("Requesting new IP certificate for {}", ip);

        let acme = self.create_acme_client().await?;

        let mut last_error = None;
        for attempt in 1..=3 {
            info!("IP certificate request attempt {}/3", attempt);

            match acme
                .obtain_ip_certificate(ip, &self.http_challenge_handler)
                .await
            {
                Ok((chain, key)) => {
                    self.storage.save_ip_certificate(&chain, &key)?;
                    self.tls.reload_ip(&chain, &key)?;

                    info!("Successfully obtained and loaded IP certificate for {}", ip);
                    return Ok(());
                }
                Err(e) => {
                    error!("IP certificate request attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < 3 {
                        let delay = Duration::from_secs(30 * (1 << (attempt - 1)));
                        info!("Retrying in {:?}", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }
}

/// Log a certificate validation result with consistent formatting
fn log_validation_result(cert_type: &str, result: &ValidationResult) {
    if result.needs_renewal {
        warn!(
            "Existing {} certificate needs renewal: {}",
            cert_type,
            result.reason.as_deref().unwrap_or("unknown")
        );
    } else {
        info!(
            "Existing {} certificate is valid (expires in {} days)",
            cert_type,
            result.days_until_expiry.unwrap_or(0)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_fields() {
        // Verify all config fields exist and have expected types
        let _ = &CONFIG.tls_enabled;
        let _ = &CONFIG.https_port;
        let _ = &CONFIG.cert_dir;
        let _ = &CONFIG.acme_email;
        let _ = &CONFIG.acme_directory;
        let _ = &CONFIG.cert_renewal_days;
        let _ = &CONFIG.cert_check_hours;
        let _ = &CONFIG.ip_cert_enabled;
        let _ = &CONFIG.ip_cert_check_hours;
        let _ = &CONFIG.ip_cert_renewal_hours;
    }
}
