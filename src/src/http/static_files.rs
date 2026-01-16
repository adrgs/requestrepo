//! Static file serving with in-memory caching and runtime config injection
//!
//! This module loads all frontend files into memory at startup and serves them
//! from memory to avoid disk I/O. It also injects runtime configuration into
//! index.html so that environment variables can be read at runtime instead of
//! build time.

use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{info, warn};

use crate::utils::config::CONFIG;

/// Cached static file with content type
#[derive(Clone)]
pub struct CachedFile {
    pub content: Vec<u8>,
    pub content_type: &'static str,
    pub cache_control: &'static str,
}

/// In-memory cache for static frontend files
#[derive(Clone)]
pub struct StaticFiles {
    files: HashMap<String, CachedFile>,
    index_html: Vec<u8>,
}

impl StaticFiles {
    /// Load all static files from the public directory into memory
    pub async fn load(public_dir: &str) -> Self {
        let mut files = HashMap::new();
        let mut index_html = Vec::new();

        let public_path = Path::new(public_dir);
        if !public_path.exists() {
            warn!("Public directory does not exist: {}", public_dir);
            return Self { files, index_html };
        }

        // Recursively load all files
        if let Err(e) = Self::load_directory(&mut files, public_path, public_path).await {
            warn!("Error loading static files: {}", e);
        }

        // Generate index.html with injected config
        if let Some(cached) = files.get("index.html") {
            index_html = Self::inject_config(&cached.content);
        }

        let total_size: usize = files.values().map(|f| f.content.len()).sum();
        info!(
            "Loaded {} static files into memory ({:.2} MB)",
            files.len(),
            total_size as f64 / 1024.0 / 1024.0
        );

        Self { files, index_html }
    }

    /// Recursively load files from a directory
    async fn load_directory(
        files: &mut HashMap<String, CachedFile>,
        base_path: &Path,
        current_path: &Path,
    ) -> Result<(), std::io::Error> {
        let mut entries = fs::read_dir(current_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                Box::pin(Self::load_directory(files, base_path, &path)).await?;
            } else if path.is_file() {
                // Get relative path from base
                let relative_path = path
                    .strip_prefix(base_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"); // Normalize Windows paths

                if let Ok(content) = fs::read(&path).await {
                    let content_type = Self::get_content_type(&path);
                    let cache_control = Self::get_cache_control(&relative_path);

                    files.insert(
                        relative_path,
                        CachedFile {
                            content,
                            content_type,
                            cache_control,
                        },
                    );
                }
            }
        }

        Ok(())
    }

    /// Inject runtime configuration into index.html
    ///
    /// This escapes values properly to prevent XSS via malicious env vars.
    /// The escaping handles:
    /// - JSON string escaping (via serde_json)
    /// - Script tag injection (`</script>` -> `<\/script>`)
    /// - HTML comment injection (`<!--` -> `<\!--`)
    fn inject_config(original_html: &[u8]) -> Vec<u8> {
        let html = String::from_utf8_lossy(original_html);

        // Build config object - only include non-sensitive values
        let config = serde_json::json!({
            "SENTRY_DSN_FRONTEND": CONFIG.sentry_dsn_frontend.as_deref().unwrap_or(""),
        });

        // Serialize to JSON (handles basic escaping)
        let json_str = serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string());

        // Additional XSS prevention:
        // - Replace </ with <\/ to prevent </script> injection
        // - Replace <!-- with <\!-- to prevent HTML comment injection
        let safe_json = json_str.replace("</", "<\\/").replace("<!--", "<\\!--");

        // Create the script tag to inject
        let config_script = format!(r#"<script>window.__CONFIG__={safe_json};</script>"#);

        // Inject before </head> or at the start of <body>
        let injected = if let Some(pos) = html.find("</head>") {
            format!("{}{}{}", &html[..pos], config_script, &html[pos..])
        } else if let Some(pos) = html.find("<body") {
            // Find the end of the body tag
            if let Some(end_pos) = html[pos..].find('>') {
                let insert_pos = pos + end_pos + 1;
                format!(
                    "{}{}{}",
                    &html[..insert_pos],
                    config_script,
                    &html[insert_pos..]
                )
            } else {
                html.to_string()
            }
        } else {
            // Fallback: prepend to document
            format!("{config_script}{html}")
        };

        injected.into_bytes()
    }

    /// Get content type based on file extension
    fn get_content_type(path: &Path) -> &'static str {
        match path.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("js") => "application/javascript; charset=utf-8",
            Some("json") => "application/json; charset=utf-8",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("ico") => "image/x-icon",
            Some("woff") => "font/woff",
            Some("woff2") => "font/woff2",
            Some("ttf") => "font/ttf",
            Some("eot") => "application/vnd.ms-fontobject",
            Some("webp") => "image/webp",
            Some("map") => "application/json",
            _ => "application/octet-stream",
        }
    }

    /// Get cache control header based on file path
    fn get_cache_control(path: &str) -> &'static str {
        if path.contains('-') && (path.ends_with(".js") || path.ends_with(".css")) {
            // Hashed assets from Vite can be cached indefinitely
            "public, max-age=31536000, immutable"
        } else if path.ends_with(".html") {
            // HTML should be revalidated
            "no-cache"
        } else {
            // Other assets cached for 1 hour
            "public, max-age=3600"
        }
    }

    /// Get a file by path, returns index.html for SPA routes
    pub fn get(&self, path: &str) -> Option<(&[u8], &'static str, &'static str)> {
        let path = path.trim_start_matches('/');

        // Empty path or root -> index.html with injected config
        if path.is_empty() {
            return Some((&self.index_html, "text/html; charset=utf-8", "no-cache"));
        }

        // Try exact match first
        if let Some(file) = self.files.get(path) {
            // For index.html, return the version with injected config
            if path == "index.html" {
                return Some((&self.index_html, file.content_type, file.cache_control));
            }
            return Some((&file.content, file.content_type, file.cache_control));
        }

        // Check if it's an asset (has extension) - return 404 for missing assets
        let has_extension = path.contains('.') && !path.ends_with('/');
        if has_extension {
            return None;
        }

        // SPA route - return index.html with injected config
        if !self.index_html.is_empty() {
            return Some((&self.index_html, "text/html; charset=utf-8", "no-cache"));
        }

        None
    }
}
