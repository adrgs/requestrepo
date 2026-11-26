use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Reads and writes the blocklist file. Saves are atomic (temp file + rename), so a crash
/// midway cannot leave a truncated file behind.
pub(super) struct Storage {
    path: PathBuf,
}

impl Storage {
    pub(super) fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            path: data_dir.as_ref().join("abuse.json"),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Creates the data directory if it does not exist yet.
    pub(super) async fn ensure_dir(&self) -> Result<()> {
        let dir = self
            .path
            .parent()
            .context("Blocklist path has no parent directory")?;
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("Failed to create data directory {}", dir.display()))
    }

    /// The saved blocklist, or `None` when none has been saved yet.
    pub(super) async fn load(&self) -> Result<Option<String>> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(text) => Ok(Some(text)),
            // A missing file is the normal state of a fresh deployment.
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to read blocklist {}: {e}",
                self.path.display()
            )),
        }
    }

    /// Moves an unusable blocklist out of the way, keeping it for inspection, so that the
    /// next save starts a fresh one. Returns where it went.
    pub(super) async fn set_aside(&self) -> Option<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let aside = self.path.with_extension(format!("json.invalid-{stamp}"));
        tokio::fs::rename(&self.path, &aside).await.ok()?;
        Some(aside)
    }

    pub(super) async fn save(&self, contents: &[u8]) -> Result<()> {
        self.ensure_dir().await?;

        let temp_path = self.path.with_extension("json.tmp");
        let written = async {
            let mut file = tokio::fs::File::create(&temp_path)
                .await
                .with_context(|| format!("Failed to create {}", temp_path.display()))?;
            file.write_all(contents)
                .await
                .context("Failed to write blocklist")?;
            file.sync_all().await.context("Failed to sync blocklist")?;
            drop(file);
            tokio::fs::rename(&temp_path, &self.path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to move {} to {}",
                        temp_path.display(),
                        self.path.display()
                    )
                })
        }
        .await;

        if written.is_err() {
            // Never leave a half-written file behind for the next save to trip over.
            let _ = tokio::fs::remove_file(&temp_path).await;
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn load_is_none_until_first_save() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path());

        assert_eq!(storage.load().await.unwrap(), None);

        storage.save(b"{}").await.unwrap();
        assert_eq!(storage.load().await.unwrap(), Some("{}".to_string()));
    }

    #[tokio::test]
    async fn save_creates_missing_directories_and_leaves_no_temp_file() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().join("nested/data"));

        storage.save(b"first").await.unwrap();
        storage.save(b"second").await.unwrap();

        assert_eq!(storage.load().await.unwrap(), Some("second".to_string()));
        let files: Vec<_> = std::fs::read_dir(dir.path().join("nested/data"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, ["abuse.json"]);
    }

    #[tokio::test]
    async fn save_fails_when_directory_is_a_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("data"), "not a directory").unwrap();
        let storage = Storage::new(dir.path().join("data"));

        assert!(storage.save(b"{}").await.is_err());
    }
}
