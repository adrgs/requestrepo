//! V2 API routes with proper REST semantics
//!
//! All routes are under `/api/v2/` prefix.

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::http::AppState;
use crate::models::{DnsRecord, DnsRecords, FileTree};
use crate::utils::{
    auth::{can_create_session, is_admin_token_required},
    generate_jwt, generate_share_jwt, get_current_timestamp,
    get_random_subdomain, verify_jwt, verify_share_jwt, write_basic_file,
};
use crate::utils::config::CONFIG;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AuthHeader {
    #[serde(default)]
    #[allow(dead_code)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginatedTokenQuery {
    pub token: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// Pagination constants
const MAX_LIMIT: usize = 1000;
const DEFAULT_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub subdomain: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub admin_token: Option<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract admin token from request body or cookie (body takes precedence)
fn extract_admin_token(body_token: Option<&str>, cookies: &CookieJar) -> Option<String> {
    // Body takes precedence
    if let Some(token) = body_token {
        return Some(token.to_string());
    }
    // Fallback to cookie
    cookies.get("admin_token").map(|c| c.value().to_string())
}

/// Build the admin token cookie string with security settings
fn build_admin_cookie(token: &str) -> String {
    let secure = if CONFIG.tls_enabled { "; Secure" } else { "" };
    format!(
        "admin_token={}; Path=/api/; Domain={}; HttpOnly; SameSite=Strict; Max-Age=2592000{}",
        token, CONFIG.server_domain, secure
    )
}

/// Extract token from query param or Authorization header
fn extract_token(query: &TokenQuery, headers: &axum::http::HeaderMap) -> Option<String> {
    // Try query param first
    if let Some(token) = &query.token {
        return Some(token.clone());
    }

    // Try Authorization header (Bearer token)
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    None
}

/// Verify token and return subdomain, or return error response
fn verify_token_or_error(
    query: &TokenQuery,
    headers: &axum::http::HeaderMap,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let token = extract_token(query, headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "No token provided".to_string(),
                code: "missing_token".to_string(),
            }),
        )
    })?;

    verify_jwt(&token).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired token".to_string(),
                code: "invalid_token".to_string(),
            }),
        )
    })
}

// ============================================================================
// Session Routes
// ============================================================================

/// POST /api/v2/sessions - Create a new session
pub async fn create_session(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: CookieJar,
    Json(body): Json<CreateSessionRequest>,
) -> Response {
    let client_ip = addr.ip().to_string();

    // Check rate limit
    if let Err(response) = check_rate_limit(&state, &client_ip).await {
        return response.into_response();
    }

    // Extract admin token from body or cookie
    let admin_token = extract_admin_token(body.admin_token.as_deref(), &cookies);

    // Check admin token if required
    if !can_create_session(admin_token.as_deref()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Admin token required",
                "code": "admin_required"
            })),
        )
            .into_response();
    }

    let subdomain = get_random_subdomain();
    let token = match generate_jwt(&subdomain) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to generate token: {}", e),
                    "code": "token_error"
                })),
            )
                .into_response();
        }
    };

    // Initialize basic file structure
    if let Err(e) = write_basic_file(&subdomain, &state.cache).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to initialize session: {}", e),
                "code": "init_error"
            })),
        )
            .into_response();
    }

    // Build response
    let mut response = (
        StatusCode::CREATED,
        Json(SessionResponse { token, subdomain }),
    )
        .into_response();

    // Set cookie if admin token was provided in body (first-time auth)
    // This persists the admin token for future session creations
    if body.admin_token.is_some() && is_admin_token_required() {
        if let Ok(cookie_value) = HeaderValue::from_str(&build_admin_cookie(body.admin_token.as_ref().unwrap())) {
            response.headers_mut().insert(SET_COOKIE, cookie_value);
        }
    }

    response
}

/// Check rate limit for session creation
async fn check_rate_limit(
    state: &AppState,
    client_ip: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let rate_limit = CONFIG.session_rate_limit;
    let window_secs = CONFIG.session_rate_window_secs;

    // Skip rate limiting if disabled
    if rate_limit == 0 {
        return Ok(());
    }

    let now = get_current_timestamp();
    let window_start = now - (window_secs as i64);
    let rate_key = format!("ratelimit:session:{client_ip}");

    // Get current count and timestamp
    let (count, last_reset) = match state.cache.get(&rate_key).await {
        Ok(Some(data)) => {
            // Format: "count:timestamp"
            let parts: Vec<&str> = data.split(':').collect();
            if parts.len() == 2 {
                let c = parts[0].parse::<u32>().unwrap_or(0);
                let t = parts[1].parse::<i64>().unwrap_or(0);
                (c, t)
            } else {
                (0, now)
            }
        }
        _ => (0, now),
    };

    // Reset counter if window has passed
    let (new_count, new_reset) = if last_reset < window_start {
        (1, now)
    } else {
        (count + 1, last_reset)
    };

    // Check if over limit
    if new_count > rate_limit {
        let retry_after = (last_reset + window_secs as i64) - now;
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "Rate limit exceeded",
                "code": "rate_limited",
                "retry_after": retry_after.max(1)
            })),
        ));
    }

    // Update counter
    let _ = state
        .cache
        .set(&rate_key, &format!("{new_count}:{new_reset}"))
        .await;

    Ok(())
}

// ============================================================================
// DNS Routes
// ============================================================================

/// GET /api/v2/dns - Get DNS records
pub async fn get_dns(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let dns_key = format!("dns:{subdomain}");
    let records = match state.cache.get(&dns_key).await {
        Ok(Some(json_str)) => {
            serde_json::from_str::<Vec<DnsRecord>>(&json_str).unwrap_or_default()
        }
        _ => Vec::new(),
    };

    (StatusCode::OK, Json(json!({ "records": records }))).into_response()
}

/// PUT /api/v2/dns - Update DNS records
pub async fn update_dns(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
    Json(records): Json<DnsRecords>,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let dns_record_types = ["A", "AAAA", "CNAME", "TXT"];

    // Validate records
    for record in &records.records {
        if !dns_record_types.contains(&record.r#type.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid record type '{}'", record.r#type),
                    "code": "invalid_record_type"
                })),
            )
                .into_response();
        }

        let domain = record.domain.to_lowercase();
        if domain.contains(|c: char| !c.is_alphanumeric() && c != '.' && c != '-') {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid characters in domain '{}'", domain),
                    "code": "invalid_domain"
                })),
            )
                .into_response();
        }
    }

    // Clear old records
    if let Ok(Some(old_records_json)) = state.cache.get(&format!("dns:{subdomain}")).await {
        if let Ok(old_records) = serde_json::from_str::<Vec<HashMap<String, String>>>(&old_records_json)
        {
            for old_record in old_records {
                if let (Some(record_type), Some(domain)) =
                    (old_record.get("type"), old_record.get("domain"))
                {
                    let _ = state
                        .cache
                        .delete(&format!("dns:{record_type}:{domain}"))
                        .await;
                }
            }
        }
    }

    // Store new records
    let mut final_records = Vec::new();
    for record in records.records {
        // Store the user's input as-is for display
        let short_domain = record.domain.to_lowercase();

        // Compute full FQDN for DNS cache key
        let fqdn = format!(
            "{}.{}.{}.",
            short_domain,
            subdomain,
            CONFIG.server_domain
        );

        let _ = state
            .cache
            .set(
                &format!("dns:{}:{}", record.r#type, fqdn),
                &record.value,
            )
            .await;

        // Store the short domain for display, not the full FQDN
        final_records.push(json!({
            "domain": short_domain,
            "type": record.r#type,
            "value": record.value,
        }));
    }

    let _ = state
        .cache
        .set(
            &format!("dns:{subdomain}"),
            &serde_json::to_string(&final_records).unwrap_or_default(),
        )
        .await;

    (StatusCode::OK, Json(json!({ "records": final_records }))).into_response()
}

// ============================================================================
// Files Routes
// ============================================================================

/// GET /api/v2/files - Get all files
pub async fn get_files(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let files_key = format!("files:{subdomain}");
    let files = match state.cache.get(&files_key).await {
        Ok(Some(json_str)) => serde_json::from_str::<FileTree>(&json_str).unwrap_or(FileTree {
            files: HashMap::new(),
        }),
        _ => FileTree {
            files: HashMap::new(),
        },
    };

    // If files are empty or missing index.html, create default
    // This handles the case where backend restarted but JWT is still valid
    if files.files.is_empty() || !files.files.contains_key("index.html") {
        let _ = write_basic_file(&subdomain, &state.cache).await;
        // Re-fetch the files after creating default
        let files = match state.cache.get(&files_key).await {
            Ok(Some(json_str)) => serde_json::from_str::<FileTree>(&json_str).unwrap_or(FileTree {
                files: HashMap::new(),
            }),
            _ => FileTree {
                files: HashMap::new(),
            },
        };
        return (StatusCode::OK, Json(files)).into_response();
    }

    (StatusCode::OK, Json(files)).into_response()
}

/// PUT /api/v2/files - Update all files
pub async fn update_files(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
    Json(files): Json<FileTree>,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let _ = state
        .cache
        .set(
            &format!("files:{subdomain}"),
            &serde_json::to_string(&files).unwrap_or_default(),
        )
        .await;

    (StatusCode::OK, Json(json!({ "message": "Files updated" }))).into_response()
}

/// GET /api/v2/files/:path - Get single file
pub async fn get_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let files_key = format!("files:{subdomain}");
    let files = match state.cache.get(&files_key).await {
        Ok(Some(json_str)) => serde_json::from_str::<FileTree>(&json_str).unwrap_or(FileTree {
            files: HashMap::new(),
        }),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "File not found",
                    "code": "not_found"
                })),
            )
                .into_response();
        }
    };

    match files.files.get(&path) {
        Some(file) => (StatusCode::OK, Json(file.clone())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "File not found",
                "code": "not_found"
            })),
        )
            .into_response(),
    }
}

// ============================================================================
// Request Routes
// ============================================================================

/// GET /api/v2/requests - List requests with pagination
///
/// Query params:
/// - token: JWT token (or use Authorization header)
/// - limit: Max results (default 100, max 1000)
/// - offset: Skip first N results (default 0)
pub async fn list_requests(
    State(state): State<AppState>,
    Query(query): Query<PaginatedTokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Extract and verify token
    let token = query.token.clone().or_else(|| {
        headers.get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string())
    });

    let subdomain = match token {
        Some(t) => match verify_jwt(&t) {
            Some(s) => s,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": "Invalid or expired token",
                        "code": "invalid_token"
                    })),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "No token provided",
                    "code": "missing_token"
                })),
            )
                .into_response();
        }
    };

    let key = format!("requests:{subdomain}");

    // Get total count (before filtering empty objects)
    let total_raw = state.cache.llen(&key).await.unwrap_or(0);

    // Apply pagination limits
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);

    // Fetch paginated range
    let requests = match state
        .cache
        .lrange(&key, offset as isize, (offset + limit - 1) as isize)
        .await
    {
        Ok(list) => list
            .into_iter()
            .filter_map(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(true))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    let has_more = offset + limit < total_raw;

    (
        StatusCode::OK,
        Json(json!({
            "requests": requests,
            "pagination": {
                "total": total_raw,
                "limit": limit,
                "offset": offset,
                "has_more": has_more
            }
        })),
    )
        .into_response()
}

/// GET /api/v2/requests/:id - Get single request
pub async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Find request by ID
    let requests = match state
        .cache
        .lrange(&format!("requests:{subdomain}"), 0, -1)
        .await
    {
        Ok(list) => list,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Request not found",
                    "code": "not_found"
                })),
            )
                .into_response();
        }
    };

    for request_json in requests {
        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&request_json) {
            if request.get("_id").and_then(|v| v.as_str()) == Some(&id) {
                return (StatusCode::OK, Json(request)).into_response();
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "Request not found",
            "code": "not_found"
        })),
    )
        .into_response()
}

/// DELETE /api/v2/requests/:id - Delete single request
pub async fn delete_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Find and delete request by ID
    let index_key = format!("request:{subdomain}:{id}");
    let index = match state.cache.get(&index_key).await {
        Ok(Some(idx)) => idx.parse::<isize>().unwrap_or(-1),
        _ => -1,
    };

    if index < 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Request not found",
                "code": "not_found"
            })),
        )
            .into_response();
    }

    // Set to empty object to mark as deleted
    let _ = state
        .cache
        .lset(&format!("requests:{subdomain}"), index, "{}")
        .await;
    let _ = state.cache.delete(&index_key).await;

    // Broadcast deletion
    let message = crate::models::CacheMessage {
        cmd: "delete_request".to_string(),
        subdomain: subdomain.clone(),
        data: json!({ "id": id }).to_string(),
    };
    let _ = state.tx.send(message);

    (
        StatusCode::OK,
        Json(json!({ "message": "Request deleted" })),
    )
        .into_response()
}

/// DELETE /api/v2/requests - Delete all requests
pub async fn delete_all_requests(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Get all request IDs first
    let requests = state
        .cache
        .lrange(&format!("requests:{subdomain}"), 0, -1)
        .await
        .unwrap_or_default();

    // Delete index keys
    for request_json in &requests {
        if let Ok(request) = serde_json::from_str::<serde_json::Value>(request_json) {
            if let Some(id) = request.get("_id").and_then(|v| v.as_str()) {
                let _ = state
                    .cache
                    .delete(&format!("request:{subdomain}:{id}"))
                    .await;
            }
        }
    }

    // Delete the requests list
    let _ = state
        .cache
        .delete(&format!("requests:{subdomain}"))
        .await;

    // Broadcast deletion
    let message = crate::models::CacheMessage {
        cmd: "delete_all".to_string(),
        subdomain: subdomain.clone(),
        data: "{}".to_string(),
    };
    let _ = state.tx.send(message);

    (
        StatusCode::OK,
        Json(json!({ "message": "All requests deleted" })),
    )
        .into_response()
}

// ============================================================================
// Request Sharing Routes
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ShareResponse {
    pub share_token: String,
}

/// POST /api/v2/requests/:id/share - Create a share token for a request
pub async fn share_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(&query, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Verify the request exists
    let requests = match state
        .cache
        .lrange(&format!("requests:{subdomain}"), 0, -1)
        .await
    {
        Ok(list) => list,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Request not found",
                    "code": "not_found"
                })),
            )
                .into_response();
        }
    };

    let mut found = false;
    for request_json in &requests {
        if let Ok(request) = serde_json::from_str::<serde_json::Value>(request_json) {
            if let Some(req_id) = request.get("_id").and_then(|v| v.as_str()) {
                if req_id == id.as_str() {
                    found = true;
                    break;
                }
            }
        }
    }

    if !found {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Request not found",
                "code": "not_found"
            })),
        )
            .into_response();
    }

    // Generate share token
    let share_token = match generate_share_jwt(&id, &subdomain) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to generate share token: {}", e),
                    "code": "token_error"
                })),
            )
                .into_response();
        }
    };

    (StatusCode::CREATED, Json(ShareResponse { share_token })).into_response()
}

/// GET /api/v2/requests/shared/:token - Get a shared request (no auth required)
pub async fn get_shared_request(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    // Verify share token
    let claims = match verify_share_jwt(&token) {
        Some(c) => c,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Invalid or expired share token",
                    "code": "invalid_share_token"
                })),
            )
                .into_response();
        }
    };

    // Find the request
    let requests = match state
        .cache
        .lrange(&format!("requests:{}", claims.subdomain), 0, -1)
        .await
    {
        Ok(list) => list,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Request not found",
                    "code": "not_found"
                })),
            )
                .into_response();
        }
    };

    for request_json in requests {
        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&request_json) {
            if request.get("_id").and_then(|v| v.as_str()) == Some(&claims.request_id) {
                return (StatusCode::OK, Json(request)).into_response();
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "Request not found",
            "code": "not_found"
        })),
    )
        .into_response()
}
