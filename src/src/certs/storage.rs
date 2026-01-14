use anyhow::{Context, Result};
use instant_acme::AccountCredentials;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info};

/// Handles certificate file storage with atomic writes
pub struct CertStorage {
    cert_dir: PathBuf,
}

impl CertStorage {
    pub fn new(cert_dir: impl Into<PathBuf>) -> Result<Self> {
        let cert_dir = cert_dir.into();

        // Create directory if it doesn't exist
        if !cert_dir.exists() {
            fs::create_dir_all(&cert_dir)
                .with_context(|| format!("Failed to create cert directory: {cert_dir:?}"))?;
            info!("Created certificate directory: {:?}", cert_dir);
        }

        Ok(Self { cert_dir })
    }

    /// Path to the full certificate chain (PEM format)
    pub fn fullchain_path(&self) -> PathBuf {
        self.cert_dir.join("fullchain.pem")
    }

    /// Path to the private key (PEM format)
    pub fn privkey_path(&self) -> PathBuf {
        self.cert_dir.join("privkey.pem")
    }

    /// Path to the ACME account credentials (JSON format)
    pub fn account_path(&self) -> PathBuf {
        self.cert_dir.join("account.json")
    }

    /// Load existing certificate and private key if they exist
    /// Returns None if either file is missing
    pub fn load_certificate(&self) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let chain_path = self.fullchain_path();
        let key_path = self.privkey_path();

        if !chain_path.exists() || !key_path.exists() {
            debug!("Certificate files not found");
            return Ok(None);
        }

        let chain = fs::read(&chain_path)
            .with_context(|| format!("Failed to read certificate chain: {chain_path:?}"))?;
        let key = fs::read(&key_path)
            .with_context(|| format!("Failed to read private key: {key_path:?}"))?;

        info!("Loaded existing certificate from {:?}", chain_path);
        Ok(Some((chain, key)))
    }

    /// Save certificate and private key atomically
    /// Uses temp file + rename to prevent corruption on crash
    pub fn save_certificate(&self, chain: &[u8], key: &[u8]) -> Result<()> {
        self.atomic_write(&self.fullchain_path(), chain)?;
        self.atomic_write(&self.privkey_path(), key)?;

        // Set restrictive permissions on private key
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let key_path = self.privkey_path();
            let mut perms = fs::metadata(&key_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&key_path, perms)?;
        }

        info!("Saved certificate to {:?}", self.fullchain_path());
        Ok(())
    }

    /// Load existing ACME account credentials or return None
    pub fn load_account(&self) -> Result<Option<AccountCredentials>> {
        let path = self.account_path();

        if !path.exists() {
            debug!("Account credentials not found");
            return Ok(None);
        }

        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read account file: {path:?}"))?;
        let creds: AccountCredentials = serde_json::from_str(&data)
            .with_context(|| "Failed to parse account credentials")?;

        info!("Loaded existing ACME account from {:?}", path);
        Ok(Some(creds))
    }

    /// Save ACME account credentials atomically
    pub fn save_account(&self, creds: &AccountCredentials) -> Result<()> {
        let data = serde_json::to_string_pretty(creds)
            .with_context(|| "Failed to serialize account credentials")?;

        self.atomic_write(&self.account_path(), data.as_bytes())?;

        // Set restrictive permissions on account file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = self.account_path();
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        info!("Saved ACME account to {:?}", self.account_path());
        Ok(())
    }

    /// Atomic write: write to temp file then rename
    fn atomic_write(&self, path: &PathBuf, data: &[u8]) -> Result<()> {
        let temp_path = path.with_extension("tmp");

        let mut file = File::create(&temp_path)
            .with_context(|| format!("Failed to create temp file: {temp_path:?}"))?;
        file.write_all(data)
            .with_context(|| format!("Failed to write temp file: {temp_path:?}"))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync temp file: {temp_path:?}"))?;

        fs::rename(&temp_path, path)
            .with_context(|| format!("Failed to rename {temp_path:?} to {path:?}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_paths() {
        let dir = TempDir::new().unwrap();
        let storage = CertStorage::new(dir.path()).unwrap();

        assert_eq!(storage.fullchain_path(), dir.path().join("fullchain.pem"));
        assert_eq!(storage.privkey_path(), dir.path().join("privkey.pem"));
        assert_eq!(storage.account_path(), dir.path().join("account.json"));
    }

    #[test]
    fn test_load_missing_certificate() {
        let dir = TempDir::new().unwrap();
        let storage = CertStorage::new(dir.path()).unwrap();

        let result = storage.load_certificate().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_certificate() {
        let dir = TempDir::new().unwrap();
        let storage = CertStorage::new(dir.path()).unwrap();

        let chain = b"-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----";
        let key = b"-----BEGIN PRIVATE KEY-----\ntest\n-----END PRIVATE KEY-----";

        storage.save_certificate(chain, key).unwrap();

        let (loaded_chain, loaded_key) = storage.load_certificate().unwrap().unwrap();
        assert_eq!(loaded_chain, chain);
        assert_eq!(loaded_key, key);
    }

    #[test]
    fn test_creates_directory() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("nested").join("certs");

        let storage = CertStorage::new(&nested).unwrap();
        assert!(nested.exists());
        assert_eq!(storage.fullchain_path(), nested.join("fullchain.pem"));
    }
}
