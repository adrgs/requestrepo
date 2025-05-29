use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use http::StatusCode;
use serde_json::{json, Value};

use crate::http::AppState;
use crate::models::{CacheMessage, TcpRequestLog};
use crate::utils::verify_jwt;

use super::TokenQuery;

pub async fn request_tcp_port(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    use crate::tcp::Server as TcpServer;
    let tcp_server = TcpServer::new(state.cache.clone(), state.tx.clone());
    
    let port = match tcp_server.allocate_port(&subdomain).await {
        Ok(port) => port.to_string(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("Failed to allocate TCP port: {}", e)})),
            )
                .into_response();
        }
    };
    
    #[cfg(not(test))]
    {
        let message = CacheMessage {
            cmd: "allocate_tcp_port".to_string(),
            subdomain: subdomain.clone(),
            data: "".to_string(),
        };

        if let Err(e) = state.tx.send(message) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("Failed to allocate TCP port: {}", e)})),
            )
                .into_response();
        }
    }

    let port_num = port.parse::<u16>().unwrap_or(0);
    
    (
        StatusCode::OK,
        Json(json!({
            "port": port_num,
            "subdomain": subdomain
        })),
    )
        .into_response()
}

pub async fn set_tcp_response(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    Json(response): Json<Value>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let response_data = match response.get("response") {
        Some(data) => match data.as_str() {
            Some(data_str) => data_str.to_string(),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": "Response must be a string"})),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "Missing 'response' field"})),
            )
                .into_response();
        }
    };

    if let Err(e) = state.cache.set(&format!("tcp_response:{}", subdomain), &response_data).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("Failed to set TCP response: {}", e)})),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({"msg": "TCP response set successfully"})),
    )
        .into_response()
}

pub async fn get_tcp_requests(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let requests = match state.cache.lrange(&format!("tcp_requests:{}", subdomain), 0, -1).await {
        Ok(requests) => {
            let mut tcp_requests = Vec::new();
            for request in requests {
                if let Ok(request) = serde_json::from_str::<TcpRequestLog>(&request) {
                    tcp_requests.push(request);
                }
            }
            tcp_requests
        }
        Err(_) => Vec::new(),
    };

    (StatusCode::OK, Json(json!({"requests": requests}))).into_response()
}

pub async fn release_tcp_port(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let message = CacheMessage {
        cmd: "release_tcp_port".to_string(),
        subdomain: subdomain.clone(),
        data: "".to_string(),
    };

    if let Err(e) = state.tx.send(message) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("Failed to release TCP port: {}", e)})),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({"msg": "TCP port released successfully"})),
    )
        .into_response()
}
