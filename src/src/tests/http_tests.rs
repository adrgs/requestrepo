#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::utils::generate_jwt;
    use serde_json::json;
    use std::sync::Arc;

    // Note: Full HTTP integration tests would require running the server
    // and making actual HTTP requests. These tests verify the cache and
    // JWT integration that the HTTP layer depends on.

    #[tokio::test]
    async fn test_jwt_generation_and_cache_integration() {
        let cache = Arc::new(Cache::new());

        let subdomain = "testsubdomain";
        let token = generate_jwt(subdomain).unwrap();

        // Verify JWT was generated (actual verification depends on env config)
        assert!(!token.is_empty());

        // Verify cache can store files for this subdomain
        let files = json!({"index.html": {"raw": "PGh0bWw+PC9odG1sPg==", "status_code": 200, "headers": []}});
        cache
            .set(&format!("files:{}", subdomain), &files.to_string())
            .await
            .unwrap();

        let result = cache.get(&format!("files:{}", subdomain)).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_dns_records_cache_integration() {
        let cache = Arc::new(Cache::new());

        let subdomain = "testsubdomain";

        // Simulate what update_dns would do
        let record_type = "A";
        let domain = format!("test.{}.requestrepo.com.", subdomain);
        let value = "1.2.3.4";

        cache
            .set(&format!("dns:{}:{}", record_type, domain), value)
            .await
            .unwrap();

        // Verify retrieval
        let result = cache
            .get(&format!("dns:{}:{}", record_type, domain))
            .await
            .unwrap();
        assert_eq!(result, Some(value.to_string()));
    }

    #[tokio::test]
    async fn test_request_logging_cache_integration() {
        let cache = Arc::new(Cache::new());

        let subdomain = "testsubdomain";
        let request_id = "test-request-id";

        // Simulate what catch_all would do
        let request_json = json!({
            "_id": request_id,
            "type": "http",
            "raw": "base64encodedrequest",
            "uid": subdomain,
            "method": "GET",
            "path": "/test",
            "headers": {},
            "date": 1234567890
        })
        .to_string();

        cache
            .rpush(&format!("requests:{}", subdomain), &request_json)
            .await
            .unwrap();
        cache
            .set(&format!("request:{}:{}", subdomain, request_id), "0")
            .await
            .unwrap();

        // Verify requests can be retrieved
        let requests = cache
            .lrange(&format!("requests:{}", subdomain), 0, -1)
            .await
            .unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains(request_id));

        // Verify request index exists
        let index = cache
            .get(&format!("request:{}:{}", subdomain, request_id))
            .await
            .unwrap();
        assert_eq!(index, Some("0".to_string()));
    }

    #[tokio::test]
    async fn test_files_storage_and_retrieval() {
        let cache = Arc::new(Cache::new());

        let subdomain = "testsubdomain";

        // Store files configuration
        let files = json!({
            "index.html": {
                "raw": "PGh0bWw+PGJvZHk+SGVsbG8gV29ybGQ8L2JvZHk+PC9odG1sPg==",
                "status_code": 200,
                "headers": [
                    {"header": "Content-Type", "value": "text/html"}
                ]
            },
            "api/data.json": {
                "raw": "eyJtZXNzYWdlIjogIkhlbGxvIn0=",
                "status_code": 200,
                "headers": [
                    {"header": "Content-Type", "value": "application/json"}
                ]
            }
        });

        cache
            .set(&format!("files:{}", subdomain), &files.to_string())
            .await
            .unwrap();

        // Retrieve and verify
        let result = cache.get(&format!("files:{}", subdomain)).await.unwrap();
        assert!(result.is_some());

        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("index.html").is_some());
        assert!(parsed.get("api/data.json").is_some());
    }

    #[tokio::test]
    async fn test_delete_request() {
        let cache = Arc::new(Cache::new());

        let subdomain = "testsubdomain";

        // Add some requests
        for i in 0..3 {
            let request_json = json!({
                "_id": format!("request-{}", i),
                "type": "http",
                "path": format!("/path{}", i)
            })
            .to_string();
            cache
                .rpush(&format!("requests:{}", subdomain), &request_json)
                .await
                .unwrap();
            cache
                .set(
                    &format!("request:{}:request-{}", subdomain, i),
                    &i.to_string(),
                )
                .await
                .unwrap();
        }

        // Verify we have 3 requests
        let requests = cache
            .lrange(&format!("requests:{}", subdomain), 0, -1)
            .await
            .unwrap();
        assert_eq!(requests.len(), 3);

        // Delete a request (set to empty object like the actual implementation)
        cache
            .lset(&format!("requests:{}", subdomain), 1, "{}")
            .await
            .unwrap();
        cache
            .delete(&format!("request:{}:request-1", subdomain))
            .await
            .unwrap();

        // Verify index key was deleted
        let index = cache
            .get(&format!("request:{}:request-1", subdomain))
            .await
            .unwrap();
        assert!(index.is_none());
    }

    #[tokio::test]
    async fn test_invalid_jwt_verification() {
        // Various invalid tokens should return None
        assert!(crate::utils::verify_jwt("invalid").is_none());
        assert!(crate::utils::verify_jwt("").is_none());
        assert!(crate::utils::verify_jwt("eyJhbGciOiJIUzI1NiJ9.invalid.invalid").is_none());
    }
}
