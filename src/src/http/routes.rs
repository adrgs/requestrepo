//! HTTP routes for request logging and file serving
//!
//! This module contains the catch-all handler that logs incoming HTTP requests
//! and serves files from the configured file tree.

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::collections::HashMap;
use std::str::FromStr;

use crate::http::AppState;
use crate::ip2country::lookup_country;
use crate::models::{HttpRequestLog, Response as ResponseModel};
use crate::utils::{
    generate_request_id, get_current_timestamp, get_subdomain_from_hostname,
    get_subdomain_from_path,
};
use serde_json::json;

/// Health check endpoint for monitoring and orchestration
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.cache.stats();

    let memory_used_mb = stats.memory_used_bytes as f64 / 1024.0 / 1024.0;
    let memory_limit_mb = stats.memory_limit_bytes as f64 / 1024.0 / 1024.0;

    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "cache": {
                "kv_entries": stats.kv_entries,
                "request_lists": stats.request_lists,
                "total_requests": stats.total_requests,
                "memory_used_mb": format!("{:.2}", memory_used_mb),
                "memory_limit_mb": format!("{:.2}", memory_limit_mb),
                "memory_usage_pct": format!("{:.1}", (memory_used_mb / memory_limit_mb) * 100.0)
            }
        })),
    )
}

/// Catch-all handler that logs HTTP requests and serves files
pub async fn catch_all(
    State(state): State<AppState>,
    connect_info: axum::extract::ConnectInfo<std::net::SocketAddr>,
    uri: Uri,
    method: axum::http::Method,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let path = uri.path();

    let subdomain =
        get_subdomain_from_hostname(host).or_else(|| get_subdomain_from_path(path));

    if let Some(subdomain) = subdomain.clone() {
        let body_bytes = body.to_vec();

        let request_id = generate_request_id();

        // Use actual connection IP - cannot be spoofed
        let client_ip = connect_info.0.ip().to_string();
        let client_port = Some(connect_info.0.port());

        let country = lookup_country(&client_ip);

        // Extract query string and build full URL
        let query_string = uri.query().map(|s| format!("?{}", s));
        let protocol = "HTTP/1.1".to_string();

        // Build full URL (fragments are not sent to server in HTTP)
        let scheme = if headers
            .get("x-forwarded-proto")
            .and_then(|h| h.to_str().ok())
            .map(|s| s == "https")
            .unwrap_or(false)
        {
            "https"
        } else {
            "http"
        };
        let full_url = format!(
            "{}://{}{}{}",
            scheme,
            host,
            path,
            query_string.as_deref().unwrap_or("")
        );

        let request_log = HttpRequestLog {
            _id: request_id.clone(),
            r#type: "http".to_string(),
            raw: BASE64.encode(&body_bytes),
            uid: subdomain.clone(),
            method: method.to_string(),
            path: path.to_string(),
            query: query_string,
            fragment: None, // Fragments are not sent to server in HTTP
            url: full_url,
            protocol,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            date: get_current_timestamp(),
            ip: Some(client_ip),
            port: client_port,
            country,
        };

        let request_json = serde_json::to_string(&request_log).unwrap_or_default();

        let _ = state
            .cache
            .rpush(&format!("requests:{}", subdomain), &request_json)
            .await;
        let _ = state
            .cache
            .set(&format!("request:{}:{}", subdomain, request_id), "0")
            .await;

        let message = crate::models::CacheMessage {
            cmd: "new_request".to_string(),
            subdomain: subdomain.clone(),
            data: request_json,
        };

        let _ = state.tx.send(message);

        return serve_file(state, subdomain, path).await;
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
        .into_response()
}

/// Serve a file from the subdomain's file tree
async fn serve_file(state: AppState, subdomain: String, path: &str) -> Response {
    let files = match state.cache.get(&format!("files:{}", subdomain)).await {
        Ok(Some(files)) => files,
        _ => "{}".to_string(),
    };

    let files: HashMap<String, ResponseModel> = serde_json::from_str(&files).unwrap_or_default();

    let file_path = path.trim_start_matches('/');
    let file_path = if file_path.is_empty() {
        "index.html"
    } else {
        file_path
    };

    if let Some(file) = files.get(file_path) {
        let content = match BASE64.decode(&file.raw) {
            Ok(content) => content,
            Err(_) => Vec::new(),
        };

        let mut response = Response::builder()
            .status(StatusCode::from_u16(file.status_code).unwrap_or(StatusCode::OK));

        for header in &file.headers {
            if let Ok(name) = HeaderName::from_str(&header.header) {
                if let Ok(value) = HeaderValue::from_str(&header.value) {
                    response = response.header(name, value);
                }
            }
        }

        return response
            .body(Body::from(content))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal server error"))
                    .unwrap()
            })
            .into_response();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
        .into_response()
}
