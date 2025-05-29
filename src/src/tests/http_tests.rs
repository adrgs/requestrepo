
#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::http::routes;
    use crate::http::AppState;
    use crate::utils::{generate_jwt, verify_jwt};
    use crate::utils::config::CONFIG;
    use axum::{
        body::Body,
        extract::{Query, State},
        http::{Method, Request, StatusCode},
        response::IntoResponse,
        Json,
    };
    use axum::body::to_bytes;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use std::collections::HashSet;
    use tokio::sync::broadcast;

    async fn setup() -> AppState {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        let _ = cache.set("test_key", "test_value").await;
        
        AppState { cache, tx }
    }
    
    fn get_valid_subdomain() -> String {
        let alphabet = CONFIG.subdomain_alphabet.chars().collect::<Vec<_>>();
        let subdomain: String = (0..CONFIG.subdomain_length)
            .map(|_| alphabet[0])
            .collect();
        
        assert!(verify_subdomain(&subdomain, CONFIG.subdomain_length, &CONFIG.subdomain_alphabet_set));
        
        subdomain
    }
    
    fn verify_subdomain(
        subdomain: &str,
        length: usize,
        alphabet_set: &HashSet<char>,
    ) -> bool {
        subdomain.len() == length && subdomain.chars().all(|c| alphabet_set.contains(&c))
    }

    #[tokio::test]
    async fn test_get_token() {
        let state = setup().await;
        
        let _request = Request::builder()
            .method(Method::POST)
            .uri("/api/get_token")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        
        let response = routes::get_token(State(state), Json(json!({}))).await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(body.get("token").is_some());
        assert!(body.get("subdomain").is_some());
        
        let subdomain = body["subdomain"].as_str().unwrap();
        let token = body["token"].as_str().unwrap();
        assert!(verify_jwt(token).is_some());
    }

    #[tokio::test]
    async fn test_update_dns() {
        let state = setup().await;
        
        let subdomain = get_valid_subdomain();
        let token = generate_jwt(&subdomain).unwrap();
        
        assert!(verify_jwt(&token).is_some());
        
        let dns_records = json!({
            "records": [
                {
                    "domain": "test",
                    "type": "A",
                    "value": "1.2.3.4"
                }
            ]
        });
        
        let response = routes::update_dns(
            State(state.clone()),
            Query(routes::TokenQuery { token }),
            Json(serde_json::from_value(dns_records).unwrap()),
        )
        .await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let dns_key = format!("dns:A:test.{}.{}.", subdomain, CONFIG.server_domain);
        let value = state.cache.get(&dns_key).await.unwrap();
        assert_eq!(value, Some("1.2.3.4".to_string()));
    }

    #[tokio::test]
    async fn test_get_dns() {
        let state = setup().await;
        
        let subdomain = get_valid_subdomain();
        let token = generate_jwt(&subdomain).unwrap();
        
        assert!(verify_jwt(&token).is_some());
        
        let dns_records = json!([
            {
                "domain": format!("test.{}.{}.", subdomain, CONFIG.server_domain),
                "type": "A",
                "value": "1.2.3.4"
            }
        ]);
        
        state.cache.set(&format!("dns:{}", subdomain), &dns_records.to_string()).await.unwrap();
        
        let response = routes::get_dns(
            State(state),
            Query(routes::TokenQuery { token }),
        )
        .await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(body, dns_records);
    }

    #[tokio::test]
    async fn test_invalid_token() {
        let state = setup().await;
        
        let response = routes::get_dns(
            State(state),
            Query(routes::TokenQuery { token: "invalid".to_string() }),
        )
        .await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    
    #[tokio::test]
    async fn test_tcp_port_request() {
        let state = setup().await;
        
        let subdomain = get_valid_subdomain();
        let token = generate_jwt(&subdomain).unwrap();
        
        assert!(verify_jwt(&token).is_some());
        
        let response = routes::tcp::request_tcp_port(
            State(state.clone()),
            Query(routes::TokenQuery { token }),
        )
        .await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(body.get("port").is_some());
        let port = body["port"].as_u64().unwrap();
        assert!(port >= CONFIG.tcp_port_range_start as u64);
        assert!(port <= CONFIG.tcp_port_range_end as u64);
    }
}
