
#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use std::time::Duration;
    use tokio::time::sleep;

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
    async fn test_exists() {
        let cache = Cache::new();
        
        let exists = cache.exists("test_key").await.unwrap();
        assert!(!exists);
        
        cache.set("test_key", "test_value").await.unwrap();
        
        let exists = cache.exists("test_key").await.unwrap();
        assert!(exists);
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
    async fn test_publish_subscribe() {
        let cache = Cache::new();
        
        let mut rx = cache.subscribe();
        
        let receivers = cache.publish("test_channel", "test_message").await.unwrap();
        assert_eq!(receivers, 1);
        
        let message = rx.try_recv().unwrap();
        assert_eq!(message.cmd, "message");
        assert_eq!(message.subdomain, "test_channel");
        assert_eq!(message.data, "test_message");
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
    async fn test_expiration() {
        
        let cache = Cache::new();
        
        cache.set("test_key", "test_value").await.unwrap();
        
        sleep(Duration::from_secs(1)).await;
        
        cache.cleanup_expired();
        
        let value = cache.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }
}
