#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::certs::TlsManager;
    use std::sync::Arc;

    fn setup_crypto_provider() {
        // Install the crypto provider for tests
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    // Self-signed test certificate and key for testing (2048-bit RSA)
    // Valid from 2026-01-12 to 2027-01-12
    const TEST_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDQzCCAiugAwIBAgIUD0hfyzae6ghDY8kydee9wNnRm98wDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQdGVzdC5leGFtcGxlLmNvbTAeFw0yNjAxMTIxMjQ4NTRa
Fw0yNzAxMTIxMjQ4NTRaMBsxGTAXBgNVBAMMEHRlc3QuZXhhbXBsZS5jb20wggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC099N9wWQobZfYJp28E0FnK3oe
XGblWEW5Cu39K7QsIRsPHE1paqchrOa4ZDh0uGe6GSc56JGfOHxZs4J5WXydF69q
Tx3r6ctamy19Ig3cavSfMNltItKcRfNxU1NvttG45Gc18uOEWVBqByaWOyIdkO2z
0FDICcyL9nQjiIH1bE2FTUyQaCUBXaYylVIzDoujDCHtEyzzWu125DABM3sDS7xF
PLW87GMtzTjJR26OOsbyD5+L25EnWMxsLoHFlIGmvsgqfibk48d0fZMEyAmxTXGo
Cx0hVxZpx2aCF7OBsavZACTZjw+l6eNVbaFfQfo8jmBGi8Pwxse6mLsWnad7AgMB
AAGjfzB9MB0GA1UdDgQWBBR55S4LvoCJPSTDtG9vJSJJDv+98jAfBgNVHSMEGDAW
gBR55S4LvoCJPSTDtG9vJSJJDv+98jAPBgNVHRMBAf8EBTADAQH/MCoGA1UdEQQj
MCGCEHRlc3QuZXhhbXBsZS5jb22CDSouZXhhbXBsZS5jb20wDQYJKoZIhvcNAQEL
BQADggEBAD+d2CLOfWBbZfxYNeHmBLegTfecZdIUyuAW5HNl/iFCGi3E+9FqEzhd
Eox1ebg+KAkuyLxQjn4m5Tm8t2FpwbpGdYXtQf40l2IKAWNiJ4owMhExC9cjWO5B
vYYwviOy88spS8Fo3VPc7WdVIfOZ3s0q6UvUOK/9O+YWisk/KxoIHWdFSwzjo2yu
qchD9o9AlilfIzxdUeUiBzNe/ekEMRKUI1zsH6rvL1Ds2j6MJyaURa0tyjc4RB/F
7oAY93M3GSt+F29zEUtZ5vSdHRgDDZQPdRbfrqUNNaqqkWgXNFomcE3XvW0/XioZ
VlcJkK5hXS72poqmocogh1KGs9O0/o4=
-----END CERTIFICATE-----"#;

    const TEST_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC099N9wWQobZfY
Jp28E0FnK3oeXGblWEW5Cu39K7QsIRsPHE1paqchrOa4ZDh0uGe6GSc56JGfOHxZ
s4J5WXydF69qTx3r6ctamy19Ig3cavSfMNltItKcRfNxU1NvttG45Gc18uOEWVBq
ByaWOyIdkO2z0FDICcyL9nQjiIH1bE2FTUyQaCUBXaYylVIzDoujDCHtEyzzWu12
5DABM3sDS7xFPLW87GMtzTjJR26OOsbyD5+L25EnWMxsLoHFlIGmvsgqfibk48d0
fZMEyAmxTXGoCx0hVxZpx2aCF7OBsavZACTZjw+l6eNVbaFfQfo8jmBGi8Pwxse6
mLsWnad7AgMBAAECggEAAeJWqbIwGG+pALoeMvgXBTuhpjFGF6XcoU7o0sBNZuw3
+5H1iF8bkFvcJxf8B9MSWWYA+jcJVRlhyg6hZswKDRn9seNS/MLOuDRyESHKonyG
ltBhDc7VVy/Z7K3Lv5etWqRzd/n9vmyuS/xOaDVYvJVmtCmQvb8ZtGyuIYnpTc8n
WVqTzztgIJJw/Jz0Gvn8qpaiSn4u6MEi6jhaqxle5j30CLMmrjMPZGfyKFfJNIsU
N28GNfi+Y07P8jaivNeeMlZxEIz+r2SP2q4DXc++AnkDjvDuj+xSyMptlWgtN0dN
oOQz70Vq026L+Z/dNt2UHTw1XeaeUkhTDnz9eyIoWQKBgQDv2m9Oq4sfUTu2R3Yr
mLbfjU7phBGUe90n8xRXo02BdqUTEiJDYTtHdsUzBlbTlQAui4kAy/nrTva5ahti
PCthTRTNzWt58KIB1lIHM7M406eudXOfImXYd4bFfyrCUtvO1mYgbd1IWieek+PJ
4SoogTl3lpUkArD/hGovY9o0hQKBgQDBJpSFCA+piaaK2UKmEP2jHWFYgNcMi/v1
B4Z55pNPXI7LuLlht/WLeD+2nVFScI456X/gCG8VAVP43FaMqSr02HyOEENcZPgk
strdTBnCpzyzFOS3VAbcuyVZqdjS31oWdxjE9kFP61JvtnfkubMnF+GAeHEZ5tbf
yxysav0r/wKBgQDSKFOk27vPme62/RB2jJ2XbQo6o2R/9LPHgD5Pt59+k0V6W0Y4
QaaeLFwHJEPJqeF55vsI/rChVBPsTCNoRI5B+DcFSDsmoghhkbdX5h3f6dwfiXvg
cFRKghhNTepaUB2WstbTDVB5R701PiG1zpuQ2AFRXliZL96EVDJyCYy0vQKBgQCK
1jSDu1umve8cdzbVL/lhOkky2Mm7lxOw+YvMK9VPgnSSM2Htv9GcQv9pQIOoAof5
UMI+Q1G1g5Lcm2ULDr516Pb/FumUjG9h0b5Iu974cEXNZANTU+7q0wrx/IiCa1HS
PCFt0qT30WscKcjcfC8utpe9RNXnjGp/yvv9Y5o/+QKBgD4Z4O31GYgrdCR3kXnD
HxS/rSa+nRK//h58oZS2Vd/KB2bzVFMe7nCsu+d2OPcV0aQt77Y+MQUwIduwpqM9
hJ4Q/8L/eW9usMn88+uELqSPmIi4HjJx93AOOcQE6SUGebx67S/jjoTU3FX21NVN
t57I39/asDr7i7haub9Q1cb0
-----END PRIVATE KEY-----"#;

    #[test]
    fn test_tls_manager_new() {
        let manager = TlsManager::new();
        // Initially no certificate should be configured
        assert!(manager.acceptor().is_none());
    }

    #[test]
    fn test_tls_manager_reload() {
        setup_crypto_provider();
        let manager = TlsManager::new();

        // Load the test certificate
        manager
            .reload(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .expect("Failed to load certificate");

        // Now acceptor should be available
        assert!(manager.acceptor().is_some());
    }

    #[test]
    fn test_tls_manager_clone_shares_state() {
        setup_crypto_provider();
        let manager = TlsManager::new();
        let cloned = manager.clone();

        // Load cert on original
        manager
            .reload(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .expect("Failed to load certificate");

        // Clone should also see the cert
        assert!(cloned.acceptor().is_some());
    }

    #[tokio::test]
    async fn test_dns_challenge_handler() {
        let cache = Arc::new(Cache::new());

        let domain = "_acme-challenge.test.example.com";
        let token = "test-token-abc123";

        // Manually set and check TXT record (simulating what DnsChallengeHandler does)
        let key = format!("dns:TXT:{}.", domain);
        cache.set(&key, token).await.expect("Failed to set TXT");

        // Verify it was set
        let value = cache.get(&key).await.expect("Failed to get TXT");
        assert_eq!(value, Some(token.to_string()));

        // Clean up
        cache.delete(&key).await.expect("Failed to delete TXT");

        let value = cache.get(&key).await.expect("Failed to get TXT");
        assert!(value.is_none());
    }

    #[test]
    fn test_config_tls_fields_exist() {
        // Verify TLS config fields are accessible
        use crate::utils::config::CONFIG;

        let _ = CONFIG.tls_enabled;
        let _ = CONFIG.https_port;
        let _ = CONFIG.cert_dir.clone();
        let _ = CONFIG.acme_email.clone();
        let _ = CONFIG.acme_directory.clone();
        let _ = CONFIG.cert_renewal_days;
        let _ = CONFIG.cert_check_hours;
    }

    #[test]
    fn test_tls_manager_invalid_cert_rejected() {
        setup_crypto_provider();
        let manager = TlsManager::new();

        // Invalid certificate data should fail
        let result = manager.reload(b"not a cert", b"not a key");
        assert!(result.is_err());
    }

    #[test]
    fn test_tls_manager_mismatched_key_rejected() {
        setup_crypto_provider();
        let manager = TlsManager::new();

        // Correct cert but wrong key format
        let result = manager.reload(TEST_CERT.as_bytes(), b"-----BEGIN RSA PRIVATE KEY-----\ninvalid\n-----END RSA PRIVATE KEY-----");
        assert!(result.is_err());
    }
}
