use anyhow::{anyhow, Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};
use trust_dns_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
use trust_dns_resolver::TokioAsyncResolver;

use crate::cache::Cache;

/// Handles DNS-01 ACME challenges by setting TXT records in the cache
pub struct DnsChallengeHandler {
    cache: Arc<Cache>,
}

impl DnsChallengeHandler {
    pub fn new(cache: Arc<Cache>) -> Self {
        Self { cache }
    }

    /// Set a TXT record for ACME challenge
    /// The domain should be the full _acme-challenge.{domain} hostname
    pub async fn set_txt(&self, challenge_domain: &str, token: &str) -> Result<()> {
        // Ensure domain ends with a dot for DNS consistency and lowercase for case-insensitive lookup
        let domain = if challenge_domain.ends_with('.') {
            challenge_domain.to_lowercase()
        } else {
            format!("{challenge_domain}.").to_lowercase()
        };

        let key = format!("dns:TXT:{domain}");
        info!("Setting ACME challenge TXT record: {} = {}", domain, token);

        self.cache
            .set(&key, token)
            .await
            .context("Failed to set TXT record in cache")?;

        Ok(())
    }

    /// Clear the TXT record after challenge is complete
    pub async fn clear_txt(&self, challenge_domain: &str) -> Result<()> {
        let domain = if challenge_domain.ends_with('.') {
            challenge_domain.to_lowercase()
        } else {
            format!("{challenge_domain}.").to_lowercase()
        };

        let key = format!("dns:TXT:{domain}");
        info!("Clearing ACME challenge TXT record: {}", domain);

        self.cache
            .delete(&key)
            .await
            .context("Failed to delete TXT record from cache")?;

        Ok(())
    }

    /// Wait for DNS propagation by querying external resolvers
    /// Uses exponential backoff with a maximum timeout
    pub async fn wait_propagation(
        &self,
        challenge_domain: &str,
        expected_token: &str,
        timeout_secs: u64,
    ) -> Result<()> {
        let domain = if challenge_domain.ends_with('.') {
            challenge_domain.to_lowercase()
        } else {
            format!("{challenge_domain}.").to_lowercase()
        };

        info!(
            "Waiting for DNS propagation of {} (timeout: {}s)",
            domain, timeout_secs
        );

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        let mut delay = Duration::from_secs(2);
        let max_delay = Duration::from_secs(30);

        // DNS resolvers to check (Google and Cloudflare)
        let resolvers = vec![
            ("8.8.8.8", "Google"),
            ("1.1.1.1", "Cloudflare"),
        ];

        loop {
            if start.elapsed() >= timeout {
                return Err(anyhow!(
                    "DNS propagation timeout after {}s for {}",
                    timeout_secs,
                    domain
                ));
            }

            // Check ALL resolvers - all must confirm for propagation to be complete
            let mut all_confirmed = true;
            let mut confirmed_resolvers = Vec::new();

            for (resolver_ip, resolver_name) in &resolvers {
                match self.query_txt(&domain, resolver_ip).await {
                    Ok(Some(value)) if value == expected_token => {
                        debug!(
                            "DNS propagation confirmed via {} for {}",
                            resolver_name, domain
                        );
                        confirmed_resolvers.push(*resolver_name);
                    }
                    Ok(Some(value)) => {
                        debug!(
                            "TXT record found but value mismatch on {}: expected '{}', got '{}'",
                            resolver_name, expected_token, value
                        );
                        all_confirmed = false;
                    }
                    Ok(None) => {
                        debug!("TXT record not found on {} for {}", resolver_name, domain);
                        all_confirmed = false;
                    }
                    Err(e) => {
                        debug!(
                            "DNS query failed on {} for {}: {}",
                            resolver_name, domain, e
                        );
                        all_confirmed = false;
                    }
                }
            }

            // All resolvers must confirm
            if all_confirmed && confirmed_resolvers.len() == resolvers.len() {
                info!(
                    "DNS propagation confirmed on all resolvers ({}) for {}",
                    confirmed_resolvers.join(", "),
                    domain
                );
                return Ok(());
            }

            // Exponential backoff
            debug!(
                "Waiting {:?} before next DNS check (confirmed: {}/{})",
                delay,
                confirmed_resolvers.len(),
                resolvers.len()
            );
            sleep(delay).await;
            delay = std::cmp::min(delay * 2, max_delay);
        }
    }

    /// Query TXT record from a specific DNS resolver
    async fn query_txt(&self, domain: &str, resolver_ip: &str) -> Result<Option<String>> {
        let ip: Ipv4Addr = resolver_ip
            .parse()
            .context("Invalid resolver IP address")?;

        let socket = SocketAddr::new(IpAddr::V4(ip), 53);
        let name_server = NameServerConfig::new(socket, Protocol::Udp);

        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(name_server);

        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_secs(5);
        opts.attempts = 1;

        let resolver = TokioAsyncResolver::tokio(resolver_config, opts);

        // Query without trailing dot for the resolver
        let query_name = domain.trim_end_matches('.');

        let response = resolver.txt_lookup(query_name).await?;

        for txt in response.iter() {
            // TXT records can have multiple strings, concatenate them
            let value: String = txt.iter().map(|s| String::from_utf8_lossy(s)).collect();
            if !value.is_empty() {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_clear_txt() {
        let cache = Arc::new(Cache::new());
        let handler = DnsChallengeHandler::new(cache.clone());

        let domain = "_acme-challenge.test.example.com";
        let token = "test-token-12345";

        // Set the TXT record
        handler.set_txt(domain, token).await.unwrap();

        // Verify it's in the cache
        let key = "dns:TXT:_acme-challenge.test.example.com.";
        let value = cache.get(key).await.unwrap();
        assert_eq!(value, Some(token.to_string()));

        // Clear the TXT record
        handler.clear_txt(domain).await.unwrap();

        // Verify it's gone
        let value = cache.get(key).await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_domain_normalization() {
        let cache = Arc::new(Cache::new());
        let handler = DnsChallengeHandler::new(cache.clone());

        // Without trailing dot
        handler
            .set_txt("_acme-challenge.example.com", "token1")
            .await
            .unwrap();

        // With trailing dot
        handler
            .set_txt("_acme-challenge.example2.com.", "token2")
            .await
            .unwrap();

        // Both should have trailing dots in the key
        let v1 = cache.get("dns:TXT:_acme-challenge.example.com.").await.unwrap();
        let v2 = cache.get("dns:TXT:_acme-challenge.example2.com.").await.unwrap();

        assert_eq!(v1, Some("token1".to_string()));
        assert_eq!(v2, Some("token2".to_string()));
    }
}
