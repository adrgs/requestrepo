use anyhow::{anyhow, Context, Result};
use arc_swap::ArcSwap;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::ServerConfig;
use std::fmt;
use std::io::BufReader;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, info};

/// SNI-based certificate resolver that serves different certs for domain vs IP connections.
/// - When SNI is present (domain name): serves domain cert
/// - When SNI is absent (IP address, per RFC 6066): serves IP cert, falls back to domain cert
struct DualCertResolver {
    domain_cert: Arc<ArcSwap<Option<Arc<CertifiedKey>>>>,
    ip_cert: Arc<ArcSwap<Option<Arc<CertifiedKey>>>>,
}

impl fmt::Debug for DualCertResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DualCertResolver")
            .field("domain_cert", &self.domain_cert.load().is_some())
            .field("ip_cert", &self.ip_cert.load().is_some())
            .finish()
    }
}

impl ResolvesServerCert for DualCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        match client_hello.server_name() {
            Some(_) => {
                // SNI present → serve domain cert
                let cert = self.domain_cert.load();
                cert.as_ref().clone()
            }
            None => {
                // No SNI → IP connection, serve IP cert with domain cert fallback
                let ip = self.ip_cert.load();
                if let Some(cert) = ip.as_ref().as_ref() {
                    debug!("Serving IP certificate (no SNI)");
                    Some(cert.clone())
                } else {
                    // Fallback to domain cert if IP cert not yet available
                    let domain = self.domain_cert.load();
                    domain.as_ref().clone()
                }
            }
        }
    }
}

/// Hot-reloadable TLS configuration manager with dual cert support.
/// Uses ArcSwap to atomically swap TLS configs without dropping connections.
pub struct TlsManager {
    config: Arc<ArcSwap<Option<Arc<ServerConfig>>>>,
    domain_cert: Arc<ArcSwap<Option<Arc<CertifiedKey>>>>,
    ip_cert: Arc<ArcSwap<Option<Arc<CertifiedKey>>>>,
}

impl TlsManager {
    /// Create a new TLS manager (starts with no config loaded)
    pub fn new() -> Self {
        Self {
            config: Arc::new(ArcSwap::new(Arc::new(None))),
            domain_cert: Arc::new(ArcSwap::new(Arc::new(None))),
            ip_cert: Arc::new(ArcSwap::new(Arc::new(None))),
        }
    }

    /// Get a TLS acceptor for accepting new connections
    /// Returns None if no certificate is configured
    pub fn acceptor(&self) -> Option<TlsAcceptor> {
        let config = self.config.load();
        config
            .as_ref()
            .as_ref()
            .map(|c| TlsAcceptor::from(c.clone()))
    }

    /// Reload the TLS configuration with new domain certificates.
    /// Backward-compatible alias for `reload_domain()` (used by tests).
    #[cfg(test)]
    pub fn reload(&self, chain_pem: &[u8], key_pem: &[u8]) -> Result<()> {
        self.reload_domain(chain_pem, key_pem)
    }

    /// Reload the domain certificate and rebuild the TLS config
    pub fn reload_domain(&self, chain_pem: &[u8], key_pem: &[u8]) -> Result<()> {
        let certified_key = build_certified_key(chain_pem, key_pem)?;
        self.domain_cert
            .store(Arc::new(Some(Arc::new(certified_key))));
        self.rebuild_config()?;
        info!("Domain TLS certificate reloaded successfully");
        Ok(())
    }

    /// Reload the IP certificate and rebuild the TLS config
    pub fn reload_ip(&self, chain_pem: &[u8], key_pem: &[u8]) -> Result<()> {
        let certified_key = build_certified_key(chain_pem, key_pem)?;
        self.ip_cert.store(Arc::new(Some(Arc::new(certified_key))));
        self.rebuild_config()?;
        info!("IP TLS certificate reloaded successfully");
        Ok(())
    }

    /// Rebuild the ServerConfig with the DualCertResolver
    fn rebuild_config(&self) -> Result<()> {
        let resolver = DualCertResolver {
            domain_cert: Arc::clone(&self.domain_cert),
            ip_cert: Arc::clone(&self.ip_cert),
        };

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));

        self.config.store(Arc::new(Some(Arc::new(config))));
        Ok(())
    }
}

impl Clone for TlsManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            domain_cert: Arc::clone(&self.domain_cert),
            ip_cert: Arc::clone(&self.ip_cert),
        }
    }
}

impl Default for TlsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a CertifiedKey from PEM-encoded certificate chain and private key
fn build_certified_key(chain_pem: &[u8], key_pem: &[u8]) -> Result<CertifiedKey> {
    let certs = parse_certificates(chain_pem)?;
    if certs.is_empty() {
        return Err(anyhow!("No certificates found in PEM data"));
    }

    let key_der = parse_private_key(key_pem)?;
    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key_der)
        .map_err(|e| anyhow!("Failed to create signing key: {:?}", e))?;

    Ok(CertifiedKey::new(certs, signing_key))
}

/// Build a rustls ServerConfig from PEM-encoded certificate chain and private key
/// (used by tests)
#[cfg(test)]
fn build_server_config(chain_pem: &[u8], key_pem: &[u8]) -> Result<ServerConfig> {
    // Parse certificate chain
    let certs = parse_certificates(chain_pem)?;
    if certs.is_empty() {
        return Err(anyhow!("No certificates found in PEM data"));
    }

    // Parse private key
    let key = parse_private_key(key_pem)?;

    // Build rustls config
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS server config")?;

    Ok(config)
}

/// Parse PEM-encoded certificates
fn parse_certificates(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(pem);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;

    Ok(certs)
}

/// Parse a PEM-encoded private key (supports RSA, PKCS8, EC)
fn parse_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(pem);

    // Try parsing as PKCS8
    let pkcs8_keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();

    if let Some(key) = pkcs8_keys.into_iter().next() {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }

    // Reset reader and try RSA
    let mut reader = BufReader::new(pem);
    let rsa_keys: Vec<_> = rustls_pemfile::rsa_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();

    if let Some(key) = rsa_keys.into_iter().next() {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }

    // Reset reader and try EC
    let mut reader = BufReader::new(pem);
    let ec_keys: Vec<_> = rustls_pemfile::ec_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();

    if let Some(key) = ec_keys.into_iter().next() {
        return Ok(PrivateKeyDer::Sec1(key));
    }

    Err(anyhow!(
        "No private key found (tried PKCS8, RSA, and EC formats)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_crypto_provider() {
        // Install the crypto provider for tests
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    // Test certificate and key for unit testing (self-signed, DO NOT use in production)
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
        assert!(manager.acceptor().is_none());
    }

    #[test]
    fn test_parse_certificates() {
        let certs = parse_certificates(TEST_CERT.as_bytes()).unwrap();
        assert_eq!(certs.len(), 1);
    }

    #[test]
    fn test_parse_private_key() {
        let key = parse_private_key(TEST_KEY.as_bytes()).unwrap();
        match key {
            PrivateKeyDer::Pkcs8(_) => {}
            _ => panic!("Expected PKCS8 key"),
        }
    }

    #[test]
    fn test_build_server_config() {
        setup_crypto_provider();
        let config = build_server_config(TEST_CERT.as_bytes(), TEST_KEY.as_bytes()).unwrap();
        // Just verify it builds successfully
        assert!(config.alpn_protocols.is_empty()); // Default config
    }

    #[test]
    fn test_tls_manager_reload() {
        setup_crypto_provider();
        let manager = TlsManager::new();
        assert!(manager.acceptor().is_none());

        manager
            .reload(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();
        assert!(manager.acceptor().is_some());
    }

    #[test]
    fn test_tls_manager_clone() {
        setup_crypto_provider();
        let manager = TlsManager::new();
        manager
            .reload(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();

        let cloned = manager.clone();

        // Both should see the same config (shared Arc)
        assert!(manager.acceptor().is_some());
        assert!(cloned.acceptor().is_some());
    }

    #[test]
    fn test_reload_domain() {
        setup_crypto_provider();
        let manager = TlsManager::new();
        assert!(manager.acceptor().is_none());

        manager
            .reload_domain(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();
        assert!(manager.acceptor().is_some());
    }

    #[test]
    fn test_reload_ip() {
        setup_crypto_provider();
        let manager = TlsManager::new();
        assert!(manager.acceptor().is_none());

        // Load IP cert (using same test cert for simplicity)
        manager
            .reload_ip(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();
        assert!(manager.acceptor().is_some());
    }

    #[test]
    fn test_reload_domain_then_ip() {
        setup_crypto_provider();
        let manager = TlsManager::new();

        manager
            .reload_domain(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();
        assert!(manager.acceptor().is_some());

        // Loading IP cert should not break existing config
        manager
            .reload_ip(TEST_CERT.as_bytes(), TEST_KEY.as_bytes())
            .unwrap();
        assert!(manager.acceptor().is_some());
    }

    #[test]
    fn test_build_certified_key() {
        setup_crypto_provider();
        let key = build_certified_key(TEST_CERT.as_bytes(), TEST_KEY.as_bytes()).unwrap();
        assert!(!key.cert.is_empty());
    }
}
