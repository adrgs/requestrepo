use anyhow::{anyhow, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};
use tracing::{error, info};

use crate::utils::config::CONFIG;

pub struct CertificateManager {
    domain: String,
    cert_path: String,
}

impl CertificateManager {
    pub fn new(domain: &str) -> Self {
        let cert_path = format!("{}{}/", CONFIG.cert_path, domain);
        
        if let Err(e) = fs::create_dir_all(&cert_path) {
            error!("Failed to create certificate directory: {}", e);
        }
        
        Self {
            domain: domain.to_string(),
            cert_path,
        }
    }
    
    pub async fn get_or_renew_certificate(&self) -> Result<(String, String)> {
        let fullchain_path = format!("{}fullchain.pem", self.cert_path);
        let privkey_path = format!("{}privkey.pem", self.cert_path);
        
        if Path::new(&fullchain_path).exists() && Path::new(&privkey_path).exists() {
            if !self.is_certificate_expiring(&fullchain_path)? {
                let fullchain = fs::read_to_string(&fullchain_path)?;
                let privkey = fs::read_to_string(&privkey_path)?;
                return Ok((fullchain, privkey));
            }
        }
        
        info!("Requesting new certificate for {}", self.domain);
        self.generate_self_signed_certificate()?;
        
        let fullchain = fs::read_to_string(&fullchain_path)?;
        let privkey = fs::read_to_string(&privkey_path)?;
        
        Ok((fullchain, privkey))
    }
    
    fn generate_self_signed_certificate(&self) -> Result<()> {
        let rcgen = rcgen::generate_simple_self_signed(vec![
            self.domain.clone(),
            format!("*.{}", self.domain),
        ])?;
        
        let fullchain_path = format!("{}fullchain.pem", self.cert_path);
        let privkey_path = format!("{}privkey.pem", self.cert_path);
        
        let mut fullchain_file = File::create(&fullchain_path)?;
        fullchain_file.write_all(rcgen.serialize_pem()?.as_bytes())?;
        
        let mut privkey_file = File::create(&privkey_path)?;
        privkey_file.write_all(rcgen.serialize_private_key_pem().as_bytes())?;
        
        info!("Self-signed certificate generated for {}", self.domain);
        
        Ok(())
    }
    
    fn is_certificate_expiring(&self, cert_path: &str) -> Result<bool> {
        let mut file = File::open(cert_path)?;
        let mut cert_data = Vec::new();
        file.read_to_end(&mut cert_data)?;
        
        let certs = rustls_pemfile::certs(&mut cert_data.as_slice())?;
        
        if certs.is_empty() {
            return Err(anyhow!("No certificates found"));
        }
        
        let now = SystemTime::now();
        let file_metadata = fs::metadata(cert_path)?;
        let file_modified = file_metadata.modified()?;
        
        if let Ok(duration) = now.duration_since(file_modified) {
            if duration > Duration::from_secs(60 * 24 * 60 * 60) {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
}
