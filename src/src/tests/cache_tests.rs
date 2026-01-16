#[cfg(test)]
mod tests {
    use crate::cache::Cache;

    #[tokio::test]
    async fn test_set_get() {
        let cache = Cache::new();

        cache.set("test_key", "test_value").await.unwrap();

        let value = cache.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = Cache::new();

        cache.set("test_key", "test_value").await.unwrap();

        let deleted = cache.delete("test_key").await.unwrap();
        assert!(deleted);

        let value = cache.get("test_key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_rpush_lrange() {
        let cache = Cache::new();

        cache.rpush("test_list", "value1").await.unwrap();
        cache.rpush("test_list", "value2").await.unwrap();
        cache.rpush("test_list", "value3").await.unwrap();

        let values = cache.lrange("test_list", 0, -1).await.unwrap();
        assert_eq!(values, vec!["value1", "value2", "value3"]);

        let values = cache.lrange("test_list", 1, 2).await.unwrap();
        assert_eq!(values, vec!["value2", "value3"]);
    }

    #[tokio::test]
    async fn test_lset() {
        let cache = Cache::new();

        cache.rpush("test_list", "value1").await.unwrap();
        cache.rpush("test_list", "value2").await.unwrap();
        cache.rpush("test_list", "value3").await.unwrap();

        cache.lset("test_list", 1, "new_value").await.unwrap();

        let values = cache.lrange("test_list", 0, -1).await.unwrap();
        assert_eq!(values, vec!["value1", "new_value", "value3"]);
    }

    #[tokio::test]
    async fn test_keys() {
        let cache = Cache::new();

        cache.set("test:key1", "value1").await.unwrap();
        cache.set("test:key2", "value2").await.unwrap();
        cache.set("other:key3", "value3").await.unwrap();

        let keys = cache.keys("test:*").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"test:key1".to_string()));
        assert!(keys.contains(&"test:key2".to_string()));
    }

    #[tokio::test]
    async fn test_compression() {
        let cache = Cache::new();

        let large_string = "a".repeat(10000);

        cache.set("large_key", &large_string).await.unwrap();

        let value = cache.get("large_key").await.unwrap();
        assert_eq!(value, Some(large_string));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = Cache::new();

        // Add some data
        cache.set("test:key1", "value1").await.unwrap();
        cache.set("test:key2", "value2").await.unwrap();
        cache.rpush("requests:test", "request1").await.unwrap();
        cache.rpush("requests:test", "request2").await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.kv_entries, 2);
        assert_eq!(stats.request_lists, 1);
        assert_eq!(stats.total_requests, 2);
        assert!(stats.memory_used_bytes > 0);
    }

    #[tokio::test]
    async fn test_memory_tracking() {
        let cache = Cache::new();

        let initial_stats = cache.stats();
        let initial_memory = initial_stats.memory_used_bytes;

        // Add data
        let large_value = "x".repeat(1000);
        cache.set("test:key", &large_value).await.unwrap();

        let after_set_stats = cache.stats();
        assert!(after_set_stats.memory_used_bytes > initial_memory);

        // Delete data
        cache.delete("test:key").await.unwrap();

        let after_delete_stats = cache.stats();
        assert!(after_delete_stats.memory_used_bytes < after_set_stats.memory_used_bytes);
    }

    #[tokio::test]
    async fn test_lrange_negative_indices() {
        let cache = Cache::new();

        cache.rpush("test_list", "value1").await.unwrap();
        cache.rpush("test_list", "value2").await.unwrap();
        cache.rpush("test_list", "value3").await.unwrap();
        cache.rpush("test_list", "value4").await.unwrap();

        // Get last 2 elements
        let values = cache.lrange("test_list", -2, -1).await.unwrap();
        assert_eq!(values, vec!["value3", "value4"]);

        // Get all from index 1 to end
        let values = cache.lrange("test_list", 1, -1).await.unwrap();
        assert_eq!(values, vec!["value2", "value3", "value4"]);
    }

    #[tokio::test]
    async fn test_lset_negative_index() {
        let cache = Cache::new();

        cache.rpush("test_list", "value1").await.unwrap();
        cache.rpush("test_list", "value2").await.unwrap();
        cache.rpush("test_list", "value3").await.unwrap();

        // Set last element
        cache.lset("test_list", -1, "new_last").await.unwrap();

        let values = cache.lrange("test_list", 0, -1).await.unwrap();
        assert_eq!(values, vec!["value1", "value2", "new_last"]);
    }

    #[tokio::test]
    async fn test_empty_list_lrange() {
        let cache = Cache::new();

        let values = cache.lrange("nonexistent_list", 0, -1).await.unwrap();
        assert!(values.is_empty());
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let cache = Cache::new();

        cache.set("test_key", "original_value").await.unwrap();
        cache.set("test_key", "updated_value").await.unwrap();

        let value = cache.get("test_key").await.unwrap();
        assert_eq!(value, Some("updated_value".to_string()));
    }

    #[tokio::test]
    async fn test_requests_list_lru_ordering() {
        let cache = Cache::new();

        // Push to list1 first
        cache.rpush("requests:list1", "value1").await.unwrap();

        // Push to list2
        cache.rpush("requests:list2", "value2").await.unwrap();

        // Access list1 again (should move it to end in LRU order)
        cache.rpush("requests:list1", "value3").await.unwrap();

        // Verify both lists have correct values
        let list1_values = cache.lrange("requests:list1", 0, -1).await.unwrap();
        assert_eq!(list1_values, vec!["value1", "value3"]);

        let list2_values = cache.lrange("requests:list2", 0, -1).await.unwrap();
        assert_eq!(list2_values, vec!["value2"]);
    }

    #[tokio::test]
    async fn test_keys_pattern_matching() {
        let cache = Cache::new();

        cache.set("files:subdomain1", "data1").await.unwrap();
        cache.set("files:subdomain2", "data2").await.unwrap();
        cache.set("dns:subdomain1", "dns_data").await.unwrap();
        cache.rpush("requests:subdomain1", "req1").await.unwrap();

        // Test files:* pattern
        let file_keys = cache.keys("files:*").await.unwrap();
        assert_eq!(file_keys.len(), 2);

        // Test *:subdomain1 pattern
        let subdomain1_keys = cache.keys("*:subdomain1").await.unwrap();
        assert!(subdomain1_keys.len() >= 2); // files:subdomain1, dns:subdomain1

        // Test specific pattern
        let specific_keys = cache.keys("dns:subdomain1").await.unwrap();
        assert_eq!(specific_keys.len(), 1);
    }
}
