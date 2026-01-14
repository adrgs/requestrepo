#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    // Note: Integration tests for DNS would require running a DNS server
    // and making actual DNS queries. These tests verify the cache integration.

    #[tokio::test]
    async fn test_dns_cache_record_storage() {
        let cache = Arc::new(Cache::new());
        let (_tx, _) = broadcast::channel::<crate::models::CacheMessage>(1024);

        // Store a DNS record in the cache
        let key = "dns:A:test.abcdefgh.example.com.";
        let value = "1.2.3.4";

        cache.set(key, value).await.unwrap();

        // Retrieve the record
        let result = cache.get(key).await.unwrap();
        assert_eq!(result, Some(value.to_string()));
    }

    #[tokio::test]
    async fn test_dns_cache_multiple_record_types() {
        let cache = Arc::new(Cache::new());

        // Store different record types
        cache
            .set("dns:A:test.example.com.", "1.2.3.4")
            .await
            .unwrap();
        cache
            .set("dns:AAAA:test.example.com.", "::1")
            .await
            .unwrap();
        cache
            .set("dns:CNAME:test.example.com.", "other.example.com.")
            .await
            .unwrap();
        cache
            .set("dns:TXT:test.example.com.", "v=spf1 include:example.com")
            .await
            .unwrap();

        // Verify all records can be retrieved
        assert_eq!(
            cache.get("dns:A:test.example.com.").await.unwrap(),
            Some("1.2.3.4".to_string())
        );
        assert_eq!(
            cache.get("dns:AAAA:test.example.com.").await.unwrap(),
            Some("::1".to_string())
        );
        assert_eq!(
            cache.get("dns:CNAME:test.example.com.").await.unwrap(),
            Some("other.example.com.".to_string())
        );
        assert_eq!(
            cache.get("dns:TXT:test.example.com.").await.unwrap(),
            Some("v=spf1 include:example.com".to_string())
        );
    }

    #[tokio::test]
    async fn test_dns_subdomain_record_storage() {
        let cache = Arc::new(Cache::new());

        // Store subdomain DNS configuration
        let subdomain = "testsubdomain";
        let records_json =
            r#"[{"domain":"test.testsubdomain.example.com.","type":"A","value":"1.2.3.4"}]"#;

        cache
            .set(&format!("dns:{subdomain}"), records_json)
            .await
            .unwrap();

        // Verify retrieval
        let result = cache.get(&format!("dns:{subdomain}")).await.unwrap();
        assert_eq!(result, Some(records_json.to_string()));
    }

    #[tokio::test]
    async fn test_dns_record_deletion() {
        let cache = Arc::new(Cache::new());

        let key = "dns:A:test.example.com.";
        cache.set(key, "1.2.3.4").await.unwrap();

        // Delete the record
        let deleted = cache.delete(key).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        let result = cache.get(key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_dns_keys_pattern() {
        let cache = Arc::new(Cache::new());

        // Store multiple DNS records
        cache.set("dns:A:a.example.com.", "1.2.3.4").await.unwrap();
        cache.set("dns:A:b.example.com.", "5.6.7.8").await.unwrap();
        cache.set("dns:AAAA:c.example.com.", "::1").await.unwrap();

        // Find all A records
        let a_keys = cache.keys("dns:A:*").await.unwrap();
        assert_eq!(a_keys.len(), 2);

        // Find all dns keys
        let all_dns_keys = cache.keys("dns:*").await.unwrap();
        assert_eq!(all_dns_keys.len(), 3);
    }
}
