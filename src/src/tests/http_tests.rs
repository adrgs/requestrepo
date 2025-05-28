
#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::http::routes;
    use crate::http::AppState;
    use crate::utils::generate_jwt;
    use axum::{
        body::Body,
        extract::{Query, State},
        http::{HeaderMap, Method, Request, StatusCode},
        response::Response,
    };
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tower::ServiceExt;

    async fn setup() -> AppState {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        AppState { cache, tx }
    }

    #[tokio::test]
    async fn test_get_token() {
        let state = setup().await;
        
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/get_token")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        
        let response = routes::get_token(State(state), Json(json!({}))).await;
        
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(body.get("token").is_some());
        assert!(body.get("subdomain").is_some());
    }

    #[tokio::test]
    async fn test_update_dns() {
        let state = setup().await;
        
        let subdomain = "testsubdomain";
        let token = generate_jwt(subdomain).unwrap();
        
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
        
        let dns_key = format!("dns:A:test.{}.{}.", subdomain, crate::utils::config::CONFIG.server_domain);
        let value = state.cache.get(&dns_key).await.unwrap();
        assert_eq!(value, Some("1.2.3.4".to_string()));
    }

    #[tokio::test]
    async fn test_get_dns() {
        let state = setup().await;
        
        let subdomain = "testsubdomain";
        let token = generate_jwt(subdomain).unwrap();
        
        let dns_records = json!([
            {
                "domain": "test.testsubdomain.example.com.",
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
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
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
}
