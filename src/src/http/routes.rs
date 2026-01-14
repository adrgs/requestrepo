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
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

use crate::http::AppState;
use crate::ip2country::lookup_country;
use crate::models::{Header, HttpRequestLog, Response as ResponseModel};
use crate::utils::{
    generate_request_id, get_current_timestamp, get_file_path_from_url, get_subdomain_from_hostname,
    get_subdomain_from_path,
};
use serde_json::json;

/// Directory for serving static dashboard files
const PUBLIC_DIR: &str = "./public";

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

    // Handle OPTIONS preflight requests for CORS
    if method == axum::http::Method::OPTIONS {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS, HEAD, PATCH")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
            .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
            .body(Body::empty())
            .unwrap()
            .into_response();
    }

    if let Some(subdomain) = subdomain.clone() {
        let body_bytes = body.to_vec();

        let request_id = generate_request_id();

        // Use actual connection IP - cannot be spoofed
        let client_ip = connect_info.0.ip().to_string();
        let client_port = Some(connect_info.0.port());

        let country = lookup_country(&client_ip);

        // Extract query string and build full URL
        let query_string = uri.query().map(|s| format!("?{s}"));
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
                .map(|(k, v)| {
                    // Title-case header names for display (e.g., "accept" -> "Accept")
                    let name = k.as_str()
                        .split('-')
                        .map(|part| {
                            let mut chars = part.chars();
                            match chars.next() {
                                Some(first) => first.to_uppercase().chain(chars).collect(),
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("-");
                    (name, v.to_str().unwrap_or("").to_string())
                })
                .collect(),
            date: get_current_timestamp(),
            ip: Some(client_ip),
            port: client_port,
            country,
        };

        let request_json = serde_json::to_string(&request_log).unwrap_or_default();

        // Push request to list and get the new length to calculate the correct index
        let list_key = format!("requests:{subdomain}");
        let index = match state.cache.rpush(&list_key, &request_json).await {
            Ok(len) => len.saturating_sub(1), // Index is length - 1 (0-based)
            Err(_) => 0,
        };

        // Store the index for this request ID (used by delete endpoint)
        let _ = state
            .cache
            .set(&format!("request:{subdomain}:{request_id}"), &index.to_string())
            .await;

        let message = crate::models::CacheMessage {
            cmd: "new_request".to_string(),
            subdomain: subdomain.clone(),
            data: request_json,
        };

        let _ = state.tx.send(message);

        // Extract the file path from the URL (for /r/subdomain/path routing)
        let file_path = get_file_path_from_url(path);
        return serve_file(state, subdomain, &file_path).await;
    }

    // No subdomain - serve static dashboard files
    serve_static_file(path).await
}

/// Serve static files from the public directory (dashboard)
/// Includes LFI protection to prevent directory traversal attacks
async fn serve_static_file(path: &str) -> Response {
    // Sanitize path: remove leading slashes, decode URL encoding is handled by axum
    let path = path.trim_start_matches('/');

    // LFI Protection: reject any path containing ".." or starting with "/"
    if path.contains("..") || path.starts_with('/') || path.contains('\0') {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("Invalid path"))
            .unwrap()
            .into_response();
    }

    // Build the file path within PUBLIC_DIR
    let public_dir = Path::new(PUBLIC_DIR).canonicalize().unwrap_or_else(|_| Path::new(PUBLIC_DIR).to_path_buf());

    let file_path = if path.is_empty() {
        public_dir.join("index.html")
    } else {
        public_dir.join(path)
    };

    // LFI Protection: ensure the resolved path is still within PUBLIC_DIR
    let canonical_path = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // File doesn't exist - for SPA routing, serve index.html for non-asset paths
            let has_extension = path.contains('.') && !path.ends_with('/');
            if has_extension {
                // Asset file not found
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from("Not found"))
                    .unwrap()
                    .into_response();
            }
            // SPA route - serve index.html
            public_dir.join("index.html")
        }
    };

    // Final LFI check: ensure path is within public directory
    if !canonical_path.starts_with(&public_dir) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("Forbidden"))
            .unwrap()
            .into_response();
    }

    // Check if it's a directory - serve index.html
    let final_path = if canonical_path.is_dir() {
        canonical_path.join("index.html")
    } else {
        canonical_path
    };

    // Read the file
    match fs::read(&final_path).await {
        Ok(content) => {
            // Determine content type from extension
            let content_type = match final_path.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("json") => "application/json; charset=utf-8",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("ico") => "image/x-icon",
                Some("woff") => "font/woff",
                Some("woff2") => "font/woff2",
                Some("ttf") => "font/ttf",
                Some("eot") => "application/vnd.ms-fontobject",
                Some("webp") => "image/webp",
                Some("map") => "application/json", // Source maps
                _ => "application/octet-stream",
            };

            // Cache static assets longer (hashed filenames from Vite)
            let cache_control = if path.contains("-") && (path.ends_with(".js") || path.ends_with(".css")) {
                // Hashed assets can be cached indefinitely
                "public, max-age=31536000, immutable"
            } else if path.ends_with(".html") {
                // HTML should be revalidated
                "no-cache"
            } else {
                // Other assets cached for 1 hour
                "public, max-age=3600"
            };

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CACHE_CONTROL, cache_control)
                .body(Body::from(content))
                .unwrap()
                .into_response()
        }
        Err(_) => {
            // File not found
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("Not found"))
                .unwrap()
                .into_response()
        }
    }
}

/// Resolve a path to a file, with cascading index.html fallback
/// 1. Try exact match
/// 2. Try path/index.html (directory index)
/// 3. Walk up the path tree looking for index.html at each level
/// 4. Final fallback: root index.html
fn resolve_file_path<'a>(
    files: &'a HashMap<String, ResponseModel>,
    path: &str,
) -> Option<&'a ResponseModel> {
    let path = path.trim_start_matches('/').trim_end_matches('/');

    // Empty path → root index.html
    if path.is_empty() {
        return files.get("index.html");
    }

    // 1. Try exact match
    if let Some(file) = files.get(path) {
        return Some(file);
    }

    // 2. Try path/index.html (directory index)
    let dir_index = format!("{path}/index.html");
    if let Some(file) = files.get(&dir_index) {
        return Some(file);
    }

    // 3. Walk up the path tree looking for index.html at each level
    let parts: Vec<&str> = path.split('/').collect();
    for i in (0..parts.len()).rev() {
        let parent_index = if i == 0 {
            "index.html".to_string()
        } else {
            format!("{}/index.html", parts[..i].join("/"))
        };
        if let Some(file) = files.get(&parent_index) {
            return Some(file);
        }
    }

    // 4. Final fallback: root index.html (should be caught by loop above, but just in case)
    files.get("index.html")
}

/// Serve a file from the subdomain's file tree
async fn serve_file(state: AppState, subdomain: String, path: &str) -> Response {
    let files_json = match state.cache.get(&format!("files:{subdomain}")).await {
        Ok(Some(files)) => files,
        _ => "{}".to_string(),
    };

    let mut files: HashMap<String, ResponseModel> = serde_json::from_str(&files_json).unwrap_or_default();

    // If files are empty or missing index.html, create it automatically
    // This handles the case where backend restarted but JWT is still valid
    if files.is_empty() || !files.contains_key("index.html") {
        let default_response = ResponseModel {
            raw: BASE64.encode("Request logged successfully."),
            headers: vec![
                Header {
                    header: "Access-Control-Allow-Origin".to_string(),
                    value: "*".to_string(),
                },
                Header {
                    header: "Content-Type".to_string(),
                    value: "text/html; charset=utf-8".to_string(),
                },
            ],
            status_code: 200,
        };
        files.insert("index.html".to_string(), default_response);

        // Save the auto-created files back to cache
        if let Ok(files_str) = serde_json::to_string(&files) {
            let _ = state.cache.set(&format!("files:{subdomain}"), &files_str).await;
        }
    }

    if let Some(file) = resolve_file_path(&files, path) {
        let content: Vec<u8> = BASE64.decode(&file.raw).unwrap_or_default();

        let mut response = Response::builder()
            .status(StatusCode::from_u16(file.status_code).unwrap_or(StatusCode::OK));

        // Apply only user-configured headers - no automatic additions
        for file_header in &file.headers {
            if let Ok(name) = HeaderName::from_str(&file_header.header) {
                if let Ok(value) = HeaderValue::from_str(&file_header.value) {
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

    // This should rarely happen since we fall back to root index.html
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
        .into_response()
}
