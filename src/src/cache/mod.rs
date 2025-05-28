
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::{Duration as StdDuration, Instant, SystemTime};
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, error, info};

use crate::models::CacheMessage;
use crate::utils::config::CONFIG;

struct CacheEntry {
    data: Vec<u8>,
    expires_at: Instant,
}

struct ListEntry {
    items: VecDeque<String>,
    expires_at: Instant,
}

pub struct Cache {
    kv_store: RwLock<HashMap<String, CacheEntry>>,
    list_store: RwLock<HashMap<String, ListEntry>>,
    tx: broadcast::Sender<CacheMessage>,
}

impl Cache {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        let cache = Self {
            kv_store: RwLock::new(HashMap::new()),
            list_store: RwLock::new(HashMap::new()),
            tx,
        };

        let cache_clone = Arc::new(cache.clone());
        tokio::spawn(async move {
            loop {
                sleep(StdDuration::from_secs(60)).await;
                cache_clone.cleanup_expired();
            }
        });

        cache
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let ttl = StdDuration::from_secs(60 * 60 * 24 * CONFIG.cache_ttl_days);
        let expires_at = Instant::now() + ttl;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(value.as_bytes())?;
        let compressed_data = encoder.finish()?;

        let mut store = self.kv_store.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        store.insert(
            key.to_string(),
            CacheEntry {
                data: compressed_data,
                expires_at,
            },
        );

        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let store = self.kv_store.read().map_err(|_| anyhow!("Failed to acquire read lock"))?;
        
        if let Some(entry) = store.get(key) {
            if entry.expires_at > Instant::now() {
                let mut decoder = GzDecoder::new(&entry.data[..]);
                let mut decompressed = String::new();
                decoder.read_to_string(&mut decompressed)?;
                
                return Ok(Some(decompressed));
            }
        }
        
        Ok(None)
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut store = self.kv_store.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        Ok(store.remove(key).is_some())
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let store = self.kv_store.read().map_err(|_| anyhow!("Failed to acquire read lock"))?;
        Ok(store.contains_key(key) && store.get(key).unwrap().expires_at > Instant::now())
    }

    pub async fn rpush(&self, key: &str, value: &str) -> Result<usize> {
        let ttl = StdDuration::from_secs(60 * 60 * 24 * CONFIG.cache_ttl_days);
        let expires_at = Instant::now() + ttl;

        let mut store = self.list_store.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        
        let entry = store.entry(key.to_string()).or_insert_with(|| ListEntry {
            items: VecDeque::new(),
            expires_at,
        });
        
        entry.items.push_back(value.to_string());
        entry.expires_at = expires_at; // Reset expiration on push
        
        Ok(entry.items.len())
    }

    pub async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let store = self.list_store.read().map_err(|_| anyhow!("Failed to acquire read lock"))?;
        
        if let Some(entry) = store.get(key) {
            if entry.expires_at > Instant::now() {
                let len = entry.items.len() as isize;
                
                let start = if start < 0 { len + start } else { start };
                let stop = if stop < 0 { len + stop } else { stop };
                
                let start = start.max(0) as usize;
                let stop = stop.min(len - 1) as usize;
                
                if start <= stop && start < len as usize {
                    return Ok(entry.items.iter().skip(start).take(stop - start + 1).cloned().collect());
                }
            }
        }
        
        Ok(Vec::new())
    }

    pub async fn lset(&self, key: &str, index: isize, value: &str) -> Result<()> {
        let mut store = self.list_store.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        
        if let Some(entry) = store.get_mut(key) {
            if entry.expires_at > Instant::now() {
                let len = entry.items.len() as isize;
                
                let index = if index < 0 { len + index } else { index };
                
                if index >= 0 && index < len {
                    entry.items[index as usize] = value.to_string();
                    return Ok(());
                }
            }
        }
        
        Err(anyhow!("List or index not found"))
    }

    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let kv_store = self.kv_store.read().map_err(|_| anyhow!("Failed to acquire read lock"))?;
        let list_store = self.list_store.read().map_err(|_| anyhow!("Failed to acquire read lock"))?;
        
        let mut result = Vec::new();
        
        let pattern = pattern.replace("*", ".*");
        let re = regex::Regex::new(&format!("^{}$", pattern))?;
        
        for key in kv_store.keys() {
            if re.is_match(key) {
                result.push(key.clone());
            }
        }
        
        for key in list_store.keys() {
            if re.is_match(key) {
                result.push(key.clone());
            }
        }
        
        Ok(result)
    }

    pub async fn publish(&self, channel: &str, message: &str) -> Result<usize> {
        let cache_message = CacheMessage {
            cmd: "message".to_string(),
            subdomain: channel.to_string(),
            data: message.to_string(),
        };
        
        let receivers = self.tx.send(cache_message)?;
        Ok(receivers)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CacheMessage> {
        self.tx.subscribe()
    }

    fn cleanup_expired(&self) {
        if let Ok(mut store) = self.kv_store.write() {
            let now = Instant::now();
            store.retain(|_, entry| entry.expires_at > now);
        }
        
        if let Ok(mut store) = self.list_store.write() {
            let now = Instant::now();
            store.retain(|_, entry| entry.expires_at > now);
        }
    }
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Self {
            kv_store: RwLock::new(HashMap::new()),
            list_store: RwLock::new(HashMap::new()),
            tx: self.tx.clone(),
        }
    }
}
