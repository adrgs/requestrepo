//! Integration tests for ACME certificate issuance using Let's Encrypt STAGING
//!
//! These tests are ignored by default because they require:
//! 1. A real domain with DNS pointing to the test server
//! 2. Network access to Let's Encrypt staging servers
//! 3. The DNS server running to handle ACME challenges
//!
//! To run these tests:
//! ```
//! ACME_TEST_DOMAIN=test.example.com \
//! ACME_TEST_EMAIL=admin@example.com \
//! cargo test --lib acme_staging -- --ignored --nocapture
//! ```
//!
//! IMPORTANT: Always use STAGING for tests! Production has strict rate limits:
//! - 50 certificates per registered domain per week
//! - 5 duplicate certificates per week
//! - 300 new orders per account per 3 hours
//!
//! Staging has much higher limits and issues test certificates that are NOT
//! trusted by browsers (which is fine for testing).

#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::certs::CertManager;
    use std::env;
    use std::sync::Arc;

    /// Let's Encrypt staging directory URL for testing
    const ACME_DIRECTORY_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

    /// Full integration test against Let's Encrypt STAGING
    ///
    /// This test will:
    /// 1. Create a new ACME account (or load existing)
    /// 2. Request a certificate for the test domain
    /// 3. Set DNS-01 challenge TXT records
    /// 4. Complete the ACME challenge
    /// 5. Download and validate the certificate
    ///
    /// Run with:
    /// ```
    /// ACME_TEST_DOMAIN=yourdomain.com ACME_TEST_EMAIL=you@email.com \
    /// cargo test test_full_certificate_issuance_staging -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn test_full_certificate_issuance_staging() {
        // Get test configuration from environment
        let domain =
            env::var("ACME_TEST_DOMAIN").expect("ACME_TEST_DOMAIN must be set for staging tests");
        let email =
            env::var("ACME_TEST_EMAIL").expect("ACME_TEST_EMAIL must be set for staging tests");

        println!("Running ACME staging test for domain: {domain}");
        println!("Using staging directory: {ACME_DIRECTORY_STAGING}");

        // Create test cache
        let cache = Arc::new(Cache::new());

        // Create temporary cert directory
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let cert_dir = temp_dir.path().to_str().unwrap().to_string();

        println!("Using temp cert directory: {cert_dir}");

        // Set required environment variables for the test
        env::set_var("TLS_ENABLED", "true");
        env::set_var("CERT_DIR", &cert_dir);
        env::set_var("ACME_EMAIL", &email);
        env::set_var("ACME_DIRECTORY", ACME_DIRECTORY_STAGING);
        env::set_var("DOMAIN", &domain);

        // Create CertManager
        let manager = CertManager::new(cache.clone())
            .await
            .expect("Failed to create CertManager");

        // Attempt to obtain a certificate
        // Note: This requires the DNS server to be running and serving
        // the _acme-challenge TXT records from the cache
        let result = manager.check_and_renew().await;

        match result {
            Ok(()) => {
                println!("Certificate obtained successfully!");

                // Verify certificate files were created
                let fullchain_path = format!("{cert_dir}/fullchain.pem");
                let privkey_path = format!("{cert_dir}/privkey.pem");

                assert!(
                    std::path::Path::new(&fullchain_path).exists(),
                    "fullchain.pem should exist"
                );
                assert!(
                    std::path::Path::new(&privkey_path).exists(),
                    "privkey.pem should exist"
                );

                // Read and print certificate info
                let cert_pem =
                    std::fs::read_to_string(&fullchain_path).expect("Failed to read certificate");
                println!("Certificate chain length: {} bytes", cert_pem.len());
            }
            Err(e) => {
                // Print detailed error for debugging
                println!("Certificate issuance failed: {e:?}");
                panic!("Expected certificate issuance to succeed");
            }
        }
    }

    /// Test that staging URL is correctly configured
    #[test]
    fn test_staging_url_is_correct() {
        assert_eq!(
            ACME_DIRECTORY_STAGING,
            "https://acme-staging-v02.api.letsencrypt.org/directory"
        );
        assert!(ACME_DIRECTORY_STAGING.contains("staging"));
    }
}
