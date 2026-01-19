use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use linked_hash_map::LinkedHashMap;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration as StdDuration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::info;

use crate::models::CacheMessage;
use crate::utils::config::CONFIG;

/// Cache entry for key-value data (compressed)
struct KvEntry {
    data: Vec<u8>,            // Gzip compressed
    uncompressed_size: usize, // Original size for tracking
}

/// List entry for request logs (ordered by insertion time for LRU)
struct RequestList {
    items: VecDeque<String>,
    total_size: usize,
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub kv_entries: usize,
    pub request_lists: usize,
    pub total_requests: usize,
    pub memory_used_bytes: usize,
    pub memory_limit_bytes: usize,
}

/// Smart cache with tiered storage:
/// - Session data (files, DNS): Per-subdomain size limits, never expires
/// - Request logs: LRU eviction under memory pressure
pub struct Cache {
    // Key-value store for session data (files, DNS records)
    // Keys: files:{subdomain}, dns:{subdomain}, dns:{type}:{domain}
    kv_store: RwLock<HashMap<String, KvEntry>>,

    // Request logs per subdomain - LRU evicted under memory pressure
    // LinkedHashMap maintains insertion order for LRU
    request_store: RwLock<LinkedHashMap<String, RequestList>>,

    // Index mapping: request:{subdomain}:{id} -> index in request list
    request_index: RwLock<HashMap<String, usize>>,

    // Per-subdomain size tracking for limits
    subdomain_sizes: RwLock<HashMap<String, usize>>,

    // Memory tracking
    current_memory: AtomicUsize,
    max_memory: AtomicUsize,

    // Broadcast channel for pub/sub
    tx: broadcast::Sender<CacheMessage>,
}

impl Cache {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);

        // Calculate max memory from container limits or config
        let max_memory = get_memory_limit();
        info!(
            "Cache initialized with max memory: {} MB",
            max_memory / 1024 / 1024
        );

        Self {
            kv_store: RwLock::new(HashMap::new()),
            request_store: RwLock::new(LinkedHashMap::new()),
            request_index: RwLock::new(HashMap::new()),
            subdomain_sizes: RwLock::new(HashMap::new()),
            current_memory: AtomicUsize::new(0),
            max_memory: AtomicUsize::new(max_memory),
            tx,
        }
    }

    /// Start the background cleanup task
    pub fn start_cleanup_task(cache: std::sync::Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(StdDuration::from_secs(60)).await;
                cache.maybe_evict_requests();
            }
        });
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let kv_entries = self.kv_store.read().map(|s| s.len()).unwrap_or(0);
        let request_lists = self.request_store.read().map(|s| s.len()).unwrap_or(0);
        let total_requests = self
            .request_store
            .read()
            .map(|s| s.values().map(|r| r.items.len()).sum())
            .unwrap_or(0);

        CacheStats {
            kv_entries,
            request_lists,
            total_requests,
            memory_used_bytes: self.current_memory.load(Ordering::Relaxed),
            memory_limit_bytes: self.max_memory.load(Ordering::Relaxed),
        }
    }

    /// Set a key-value pair (for files, DNS records)
    /// Enforces per-subdomain size limits for session data
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let uncompressed_size = value.len();

        // Check per-subdomain limit for session data (files:*, dns:*)
        if let Some(subdomain) = extract_subdomain_from_key(key) {
            let current_size = self.get_subdomain_size(&subdomain);
            if current_size + uncompressed_size > CONFIG.max_subdomain_size_bytes {
                return Err(anyhow!(
                    "Subdomain {} storage limit exceeded ({} bytes max)",
                    subdomain,
                    CONFIG.max_subdomain_size_bytes
                ));
            }
        }

        // Compress the data
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(value.as_bytes())?;
        let compressed_data = encoder.finish()?;
        let compressed_size = compressed_data.len();

        // Update memory tracking
        let mut store = self.kv_store.write().map_err(|_| anyhow!("Lock error"))?;

        // If key exists, subtract old size
        if let Some(old_entry) = store.get(key) {
            self.current_memory
                .fetch_sub(old_entry.data.len(), Ordering::Relaxed);
            if let Some(subdomain) = extract_subdomain_from_key(key) {
                self.update_subdomain_size(&subdomain, -(old_entry.uncompressed_size as isize));
            }
        }

        // Insert new entry
        store.insert(
            key.to_string(),
            KvEntry {
                data: compressed_data,
                uncompressed_size,
            },
        );

        self.current_memory
            .fetch_add(compressed_size, Ordering::Relaxed);
        if let Some(subdomain) = extract_subdomain_from_key(key) {
            self.update_subdomain_size(&subdomain, uncompressed_size as isize);
        }

        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let store = self.kv_store.read().map_err(|_| anyhow!("Lock error"))?;

        if let Some(entry) = store.get(key) {
            let mut decoder = GzDecoder::new(&entry.data[..]);
            let mut decompressed = String::new();
            decoder.read_to_string(&mut decompressed)?;
            return Ok(Some(decompressed));
        }

        Ok(None)
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<bool> {
        // Try kv_store first
        {
            let mut store = self.kv_store.write().map_err(|_| anyhow!("Lock error"))?;
            if let Some(entry) = store.remove(key) {
                self.current_memory
                    .fetch_sub(entry.data.len(), Ordering::Relaxed);
                if let Some(subdomain) = extract_subdomain_from_key(key) {
                    self.update_subdomain_size(&subdomain, -(entry.uncompressed_size as isize));
                }
                return Ok(true);
            }
        }

        // Try request_store (for requests:{subdomain} keys)
        {
            let mut store = self
                .request_store
                .write()
                .map_err(|_| anyhow!("Lock error"))?;
            if let Some(entry) = store.remove(key) {
                self.current_memory
                    .fetch_sub(entry.total_size, Ordering::Relaxed);
                return Ok(true);
            }
        }

        // Also check request index
        let mut index = self
            .request_index
            .write()
            .map_err(|_| anyhow!("Lock error"))?;
        Ok(index.remove(key).is_some())
    }

    /// Push to a request list (right push)
    /// This is used for request logs - LRU evicted under memory pressure
    pub async fn rpush(&self, key: &str, value: &str) -> Result<usize> {
        let value_size = value.len();

        // Maybe evict old requests if memory is high
        self.maybe_evict_requests();

        let mut store = self
            .request_store
            .write()
            .map_err(|_| anyhow!("Lock error"))?;

        // Insert if not exists
        if !store.contains_key(key) {
            store.insert(
                key.to_string(),
                RequestList {
                    items: VecDeque::new(),
                    total_size: 0,
                },
            );
        }

        // Now get_refresh to move to end (most recently used) and modify
        let (len, evicted_size) = if let Some(entry) = store.get_refresh(key) {
            entry.items.push_back(value.to_string()); // O(1)
            entry.total_size += value_size;

            // Enforce per-session limit - O(1) pop if over limit
            let mut evicted = 0usize;
            if entry.items.len() > CONFIG.max_requests_per_session {
                if let Some(old) = entry.items.pop_front() {
                    // O(1)
                    let old_size = old.len();
                    entry.total_size = entry.total_size.saturating_sub(old_size);
                    evicted = old_size;
                }
            }

            (entry.items.len(), evicted)
        } else {
            return Err(anyhow!("Failed to insert into request store"));
        };

        // Update memory tracking: add new, subtract evicted
        self.current_memory.fetch_add(value_size, Ordering::Relaxed);
        if evicted_size > 0 {
            self.current_memory
                .fetch_sub(evicted_size, Ordering::Relaxed);
        }

        Ok(len)
    }

    /// Get range from list
    pub async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let store = self
            .request_store
            .read()
            .map_err(|_| anyhow!("Lock error"))?;

        if let Some(entry) = store.get(key) {
            let len = entry.items.len() as isize;

            let start = if start < 0 {
                (len + start).max(0)
            } else {
                start
            };
            let stop = if stop < 0 { len + stop } else { stop };

            let start = start as usize;
            let stop = (stop as usize).min(entry.items.len().saturating_sub(1));

            if start <= stop && start < entry.items.len() {
                return Ok(entry
                    .items
                    .iter()
                    .skip(start)
                    .take(stop - start + 1)
                    .cloned()
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    /// Get length of list
    pub async fn llen(&self, key: &str) -> Result<usize> {
        let store = self
            .request_store
            .read()
            .map_err(|_| anyhow!("Lock error"))?;
        Ok(store.get(key).map(|e| e.items.len()).unwrap_or(0))
    }

    /// Set value at index in list
    pub async fn lset(&self, key: &str, index: isize, value: &str) -> Result<()> {
        let mut store = self
            .request_store
            .write()
            .map_err(|_| anyhow!("Lock error"))?;

        if let Some(entry) = store.get_mut(key) {
            let len = entry.items.len() as isize;
            let index = if index < 0 { len + index } else { index };

            if index >= 0 && index < len {
                let old_size = entry.items[index as usize].len();
                let new_size = value.len();

                entry.items[index as usize] = value.to_string();
                entry.total_size = entry.total_size - old_size + new_size;

                self.current_memory.fetch_sub(old_size, Ordering::Relaxed);
                self.current_memory.fetch_add(new_size, Ordering::Relaxed);

                return Ok(());
            }
        }

        Err(anyhow!("List or index not found"))
    }

    /// Get keys matching pattern (used in tests)
    #[allow(dead_code)]
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let kv_store = self.kv_store.read().map_err(|_| anyhow!("Lock error"))?;
        let request_store = self
            .request_store
            .read()
            .map_err(|_| anyhow!("Lock error"))?;

        let mut result = Vec::new();

        let pattern = pattern.replace("*", ".*");
        let re = regex::Regex::new(&format!("^{pattern}$"))?;

        for key in kv_store.keys() {
            if re.is_match(key) {
                result.push(key.clone());
            }
        }

        for key in request_store.keys() {
            if re.is_match(key) {
                result.push(key.clone());
            }
        }

        Ok(result)
    }

    // --- Private helper methods ---

    fn get_subdomain_size(&self, subdomain: &str) -> usize {
        self.subdomain_sizes
            .read()
            .ok()
            .and_then(|s| s.get(subdomain).copied())
            .unwrap_or(0)
    }

    fn update_subdomain_size(&self, subdomain: &str, delta: isize) {
        if let Ok(mut sizes) = self.subdomain_sizes.write() {
            let entry = sizes.entry(subdomain.to_string()).or_insert(0);
            if delta >= 0 {
                *entry = entry.saturating_add(delta as usize);
            } else {
                *entry = entry.saturating_sub((-delta) as usize);
            }
        }
    }

    /// Evict oldest request lists if memory usage is above threshold
    fn maybe_evict_requests(&self) {
        let current = self.current_memory.load(Ordering::Relaxed);
        let max = self.max_memory.load(Ordering::Relaxed);

        // Start evicting at configured threshold (default 70%)
        let threshold = (max as f64 * CONFIG.cache_max_memory_pct) as usize;

        if current > threshold {
            // Evict enough to get below threshold, plus 10% extra to avoid
            // triggering eviction again immediately
            let extra_buffer = max / 10; // 10% of max memory
            let bytes_to_free = (current - threshold) + extra_buffer;
            self.evict_oldest_requests(bytes_to_free);
        }
    }

    /// Evict oldest individual requests to free up `bytes_to_free` bytes
    /// Uses round-robin across all subdomain lists to be fair
    fn evict_oldest_requests(&self, bytes_to_free: usize) {
        let mut freed = 0usize;
        let mut evicted_count = 0usize;
        let mut empty_lists: Vec<String> = Vec::new();

        if let Ok(mut store) = self.request_store.write() {
            // Keep evicting until we've freed enough memory
            while freed < bytes_to_free {
                let mut made_progress = false;

                // Get all keys (we need to collect to avoid borrow issues)
                let keys: Vec<String> = store.keys().cloned().collect();

                // Round-robin: pop one oldest request from each list
                for key in &keys {
                    if freed >= bytes_to_free {
                        break;
                    }

                    if let Some(entry) = store.get_mut(key) {
                        if let Some(oldest_request) = entry.items.pop_front() {
                            let request_size = oldest_request.len();
                            entry.total_size = entry.total_size.saturating_sub(request_size);
                            freed += request_size;
                            evicted_count += 1;
                            made_progress = true;

                            // Track empty lists for cleanup
                            if entry.items.is_empty() {
                                empty_lists.push(key.clone());
                            }
                        }
                    }
                }

                // If no progress was made (all lists empty), break to avoid infinite loop
                if !made_progress {
                    break;
                }
            }

            // Remove empty lists
            for key in &empty_lists {
                store.remove(key);
            }
        }

        if evicted_count > 0 {
            self.current_memory.fetch_sub(freed, Ordering::Relaxed);
            info!(
                "Evicted {} individual requests ({} bytes freed), removed {} empty lists",
                evicted_count,
                freed,
                empty_lists.len()
            );
        }
    }
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        // Clone shares the broadcast sender but creates new empty stores
        // This is intentional - use Arc<Cache> to share actual data
        Self {
            kv_store: RwLock::new(HashMap::new()),
            request_store: RwLock::new(LinkedHashMap::new()),
            request_index: RwLock::new(HashMap::new()),
            subdomain_sizes: RwLock::new(HashMap::new()),
            current_memory: AtomicUsize::new(0),
            max_memory: AtomicUsize::new(self.max_memory.load(Ordering::Relaxed)),
            tx: self.tx.clone(),
        }
    }
}

/// Extract subdomain from cache key for size tracking
fn extract_subdomain_from_key(key: &str) -> Option<String> {
    // files:{subdomain} -> subdomain
    // dns:{subdomain} -> subdomain
    if key.starts_with("files:") || key.starts_with("file:") {
        return Some(key.split(':').nth(1)?.to_string());
    }
    if key.starts_with("dns:") {
        // dns:{subdomain} or dns:{type}:{domain}
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() == 2 {
            return Some(parts[1].to_string());
        }
        // dns:{type}:{domain} - extract subdomain from domain
        // e.g., dns:A:test.abc123.example.com. -> abc123
        if parts.len() >= 3 {
            let domain = parts[2];
            // Try to extract subdomain from domain name
            let domain_parts: Vec<&str> = domain.split('.').collect();
            if domain_parts.len() >= 2 {
                // Second-to-last part before the base domain might be the subdomain
                return Some(domain_parts[1].to_string());
            }
        }
    }
    None
}

/// Get container memory limit from cgroups or system
fn get_memory_limit() -> usize {
    // Try cgroups v2
    if let Ok(content) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
        if let Ok(limit) = content.trim().parse::<usize>() {
            if limit < usize::MAX / 2 {
                return ((limit as f64) * CONFIG.cache_max_memory_pct) as usize;
            }
        }
    }

    // Try cgroups v1
    if let Ok(content) = std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
        if let Ok(limit) = content.trim().parse::<usize>() {
            if limit < usize::MAX / 2 {
                return ((limit as f64) * CONFIG.cache_max_memory_pct) as usize;
            }
        }
    }

    // Default: 1GB
    1024 * 1024 * 1024
}
