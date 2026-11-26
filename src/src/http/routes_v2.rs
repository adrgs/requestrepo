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
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use crate::abuse::{routes::suspended_rejection, AbuseList};
use crate::cache::Cache;
use crate::http::AppState;
use crate::models::{DnsRecord, DnsRecords, FileTree};
use crate::utils::config::CONFIG;
use crate::utils::{
    auth::{bearer_token, can_create_session, extract_admin_token, is_admin_token_required},
    generate_jwt, generate_share_jwt, get_current_timestamp, get_random_subdomain, verify_jwt,
    verify_share_jwt, write_basic_file,
};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginatedTokenQuery {
    pub token: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Only requests captured after the request with this ID (a cursor), or `now` for only
    /// requests that have not arrived yet
    pub after: Option<String>,
    /// Only the newest N matching requests (still oldest first)
    pub last: Option<usize>,
    /// `full` (default) or `summary`: one compact line per request, for reading at a glance
    pub view: Option<String>,
    /// Seconds to wait for a new request when there is none yet (long polling)
    pub wait: Option<u64>,
    /// Only requests of this type: http, dns, smtp or tcp
    pub r#type: Option<String>,
}

/// `scheme://host[:port]` as clients reach this server, with the port only when it is not
/// the scheme's default.
pub(crate) fn public_origin(host: &str) -> String {
    let (scheme, port, default_port) = if CONFIG.tls_enabled {
        ("https", CONFIG.https_port, 443)
    } else {
        ("http", CONFIG.http_port, 80)
    };
    if port == default_port {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}:{port}")
    }
}

/// Parses a JSON request body, answering every problem with a JSON error, as the rest of the
/// API does. With `require_content_type`, the body must be sent as `application/json`: a
/// cross-site form cannot send that without a CORS preflight.
pub(crate) fn json_body<T: serde::de::DeserializeOwned>(
    headers: &axum::http::HeaderMap,
    body: &[u8],
    require_content_type: bool,
) -> Result<T, Box<Response>> {
    let is_json = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .trim()
                .to_ascii_lowercase()
                .starts_with("application/json")
        });
    if require_content_type && !is_json {
        return Err(Box::new(
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(json!({
                    "error": "Send the body as JSON with Content-Type: application/json",
                    "code": "unsupported_media_type"
                })),
            )
                .into_response(),
        ));
    }
    serde_json::from_slice(body).map_err(|e| {
        Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid JSON body: {e}"),
                    "code": "invalid_body"
                })),
            )
                .into_response(),
        )
    })
}

/// Long polls held open at once, in total and per session. Each holds a connection (and,
/// over HTTPS, one of a fixed number of connection permits), so they must be bounded. Past
/// either limit a request is answered at once, as if it had not asked to wait.
const MAX_LONG_POLLS: usize = 256;
const MAX_LONG_POLLS_PER_SESSION: usize = 4;

/// Counts a long poll in, and out again when dropped.
struct LongPollSlot {
    subdomain: String,
}

fn long_polls() -> &'static std::sync::Mutex<HashMap<String, usize>> {
    static POLLS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, usize>>> =
        std::sync::OnceLock::new();
    POLLS.get_or_init(Default::default)
}

impl LongPollSlot {
    fn acquire(subdomain: &str) -> Option<Self> {
        let mut polls = long_polls().lock().unwrap_or_else(|e| e.into_inner());
        let total: usize = polls.values().sum();
        let mine = polls.get(subdomain).copied().unwrap_or(0);
        if total >= MAX_LONG_POLLS || mine >= MAX_LONG_POLLS_PER_SESSION {
            return None;
        }
        polls.insert(subdomain.to_string(), mine + 1);
        Some(Self {
            subdomain: subdomain.to_string(),
        })
    }
}

impl Drop for LongPollSlot {
    fn drop(&mut self) {
        let mut polls = long_polls().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = polls.get_mut(&self.subdomain) {
            *count -= 1;
            if *count == 0 {
                polls.remove(&self.subdomain);
            }
        }
    }
}

/// Longest a client may hold a long poll open.
const MAX_WAIT_SECS: u64 = 60;

// Pagination constants
const MAX_LIMIT: usize = 1000;
const DEFAULT_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub subdomain: String,
    /// Hostname that captures HTTP, DNS and SMTP traffic for this session
    pub domain: String,
    /// Base URL for HTTP traffic
    pub url: String,
    /// Any address at this domain is captured, e.g. `anything@<domain>`
    pub email: String,
}

impl SessionResponse {
    fn new(token: String, subdomain: String) -> Self {
        let domain = format!("{subdomain}.{}", CONFIG.server_domain);
        Self {
            url: format!("{}/", public_origin(&domain)),
            email: format!("anything@{domain}"),
            domain,
            token,
            subdomain,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateSessionRequest {
    pub admin_token: Option<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// How many times `pick_subdomain` draws before giving up. A draw only has to miss the
/// blocklist, so even a blocklist holding every entry it can would be missed almost
/// immediately; the bound exists so a misconfigured tiny label space cannot spin forever.
const SUBDOMAIN_DRAWS: usize = 16;

/// A fresh session label that is not suspended.
fn pick_subdomain(abuse: &AbuseList) -> Option<String> {
    pick_unblocked(get_random_subdomain, |subdomain| {
        abuse.is_blocked(subdomain)
    })
}

/// Draws labels until one is not blocked, giving up after `SUBDOMAIN_DRAWS`.
fn pick_unblocked(
    mut draw: impl FnMut() -> String,
    is_blocked: impl Fn(&str) -> bool,
) -> Option<String> {
    (0..SUBDOMAIN_DRAWS)
        .map(|_| draw())
        .find(|subdomain| !is_blocked(subdomain))
}

/// Build the admin token cookie string with security settings
fn build_admin_cookie(token: &str) -> String {
    let secure = if CONFIG.tls_enabled { "; Secure" } else { "" };
    format!("admin_token={token}; Path=/api/; HttpOnly; SameSite=Strict; Max-Age=2592000{secure}")
}

/// Verify token (query param or `Authorization: Bearer`) and return subdomain, or return
/// error response
///
/// A token for a blocked subdomain is rejected here rather than in `verify_jwt` itself:
/// to the caller it must read as "blocked", not as "invalid", because an invalid token
/// is one the frontend discards.
fn verify_token_or_error(
    token: Option<&str>,
    headers: &axum::http::HeaderMap,
    abuse: &AbuseList,
) -> Result<String, Box<Response>> {
    // Query param first, then Authorization header
    let token = token
        .map(str::to_string)
        .or_else(|| bearer_token(headers))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            Box::new(token_error(
                StatusCode::UNAUTHORIZED,
                "No token provided",
                "missing_token",
            ))
        })?;

    let subdomain = verify_jwt(&token).ok_or_else(|| {
        Box::new(token_error(
            StatusCode::UNAUTHORIZED,
            "Invalid or expired token",
            "invalid_token",
        ))
    })?;

    if abuse.is_blocked(&subdomain) {
        return Err(Box::new(suspended_rejection()));
    }
    Ok(subdomain)
}

fn token_error(status: StatusCode, error: &str, code: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
            code: code.to_string(),
        }),
    )
        .into_response()
}

/// A page of a subdomain's request log: the number of stored entries (including deleted
/// ones, which are kept as empty objects) and the parsed, non-deleted requests.
pub(crate) async fn paginated_requests(
    cache: &Cache,
    subdomain: &str,
    limit: usize,
    offset: usize,
) -> (usize, Vec<serde_json::Value>) {
    let key = format!("requests:{subdomain}");
    let total = cache.llen(&key).await.unwrap_or(0);
    let requests = cache
        .lrange(&key, offset as isize, (offset + limit - 1) as isize)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(true))
        .collect();
    (total, requests)
}

/// Deletes the per-record entries (`dns:{type}:{fqdn}`) that the DNS server answers from.
/// The subdomain's record list (`dns:{subdomain}`) is left for the caller to replace or delete.
pub(crate) async fn delete_dns_record_entries(cache: &Cache, subdomain: &str) {
    let records = cache
        .get(&format!("dns:{subdomain}"))
        .await
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Vec<DnsRecord>>(&json).ok())
        .unwrap_or_default();

    for record in &records {
        let fqdn = format!("{}.{}.{}.", record.domain, subdomain, CONFIG.server_domain);
        let _ = cache.delete(&format!("dns:{}:{fqdn}", record.r#type)).await;
    }
}

/// Validate a DNS domain name for security and RFC compliance
///
/// Security: Blocks empty domains and "@" (root subdomain) to prevent
/// users from setting DNS records for their root subdomain, which could
/// allow TLS certificate issuance via DNS-01 challenges.
///
/// Allowed:
/// - "*" for wildcard (matches all unmatched sub-subdomains)
/// - Simple subdomains like "www", "api"
/// - Nested subdomains like "a.b.c"
/// - Underscores for things like "_acme-challenge"
fn validate_dns_domain(domain: &str) -> Result<String, String> {
    let domain = domain.trim().trim_matches('.').to_lowercase();

    // Block empty domain
    if domain.is_empty() {
        return Err("Domain cannot be empty".to_string());
    }

    // Block @ (root subdomain - security risk for cert issuance)
    if domain == "@" {
        return Err("Cannot set DNS for root subdomain (security risk)".to_string());
    }

    // Allow * for wildcard
    if domain == "*" {
        return Ok("*".to_string());
    }

    // Character validation (allow _ for things like _acme-challenge)
    if domain.contains(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_') {
        return Err(format!("Invalid characters in domain '{domain}'"));
    }

    // Label length validation (DNS spec: max 63 chars per label)
    for label in domain.split('.') {
        if label.is_empty() {
            return Err("Domain contains empty label".to_string());
        }
        if label.len() > 63 {
            return Err(format!("Label '{label}' exceeds 63 characters"));
        }
    }

    // Total length validation (DNS spec: max 253 chars)
    if domain.len() > 253 {
        return Err("Domain exceeds 253 characters".to_string());
    }

    Ok(domain)
}

/// Validate a DNS record value based on record type
fn validate_dns_value(record_type: &str, value: &str) -> Result<(), String> {
    match record_type {
        "A" => {
            // Must be valid IPv4 address(es) separated by %
            for ip in value.split('%') {
                ip.parse::<Ipv4Addr>()
                    .map_err(|_| format!("Invalid IPv4 address '{ip}'"))?;
            }
            Ok(())
        }
        "AAAA" => {
            // Must be valid IPv6 address(es) separated by %
            for ip in value.split('%') {
                ip.parse::<Ipv6Addr>()
                    .map_err(|_| format!("Invalid IPv6 address '{ip}'"))?;
            }
            Ok(())
        }
        "CNAME" => {
            if value.is_empty() {
                return Err("Value cannot be empty".to_string());
            }
            Ok(())
        }
        "TXT" => {
            if value.is_empty() {
                return Err("Value cannot be empty".to_string());
            }
            if value.len() > 10000 {
                return Err("TXT records cannot exceed 10000 characters".to_string());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ============================================================================
// Session Routes
// ============================================================================

/// POST /api/v2/sessions - Create a new session
pub async fn create_session(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: CookieJar,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let client_ip = addr.ip().to_string();

    // The body is optional, so `curl -X POST .../api/v2/sessions` is enough
    let body: CreateSessionRequest = if body.iter().all(u8::is_ascii_whitespace) {
        CreateSessionRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(body) => body,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!("Body must be empty or a JSON object: {e}"),
                        "code": "invalid_body"
                    })),
                )
                    .into_response()
            }
        }
    };

    // Check rate limit
    if let Err(response) = check_rate_limit(&state, &client_ip).await {
        return response.into_response();
    }

    // Extract admin token from body, cookie or Authorization header
    let admin_token = extract_admin_token(body.admin_token.as_deref(), &cookies, &headers);

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

    // A blocked subdomain must never be handed out as a fresh session
    let Some(subdomain) = pick_subdomain(&state.abuse) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "Could not allocate a subdomain",
                "code": "subdomain_unavailable"
            })),
        )
            .into_response();
    };
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
        Json(SessionResponse::new(token, subdomain)),
    )
        .into_response();

    // Set cookie if admin token was provided in body (first-time auth)
    // This persists the admin token for future session creations
    if let Some(ref admin_token) = body.admin_token {
        if is_admin_token_required() {
            if let Ok(cookie_value) = HeaderValue::from_str(&build_admin_cookie(admin_token)) {
                response.headers_mut().insert(SET_COOKIE, cookie_value);
            }
        }
    }

    response
}

/// Generic cache-based rate limiter
async fn check_rate_limit_generic(
    state: &AppState,
    client_ip: &str,
    key_prefix: &str,
    rate_limit: u32,
    window_secs: u64,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if rate_limit == 0 {
        return Ok(());
    }

    let now = get_current_timestamp();
    let window_start = now - (window_secs as i64);
    let rate_key = format!("ratelimit:{key_prefix}:{client_ip}");

    // Get current count and timestamp (format: "count:timestamp")
    let (count, last_reset) = match state.cache.get(&rate_key).await {
        Ok(Some(data)) => {
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

    let (new_count, new_reset) = if last_reset < window_start {
        (1, now)
    } else {
        (count + 1, last_reset)
    };

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

    let _ = state
        .cache
        .set(&rate_key, &format!("{new_count}:{new_reset}"))
        .await;

    Ok(())
}

/// Check rate limit for session creation
async fn check_rate_limit(
    state: &AppState,
    client_ip: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    check_rate_limit_generic(
        state,
        client_ip,
        "session",
        CONFIG.session_rate_limit,
        CONFIG.session_rate_window_secs,
    )
    .await
}

/// Check rate limit for share token creation
async fn check_share_rate_limit(
    state: &AppState,
    client_ip: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    check_rate_limit_generic(state, client_ip, "share", 10, 60).await
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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    let dns_key = format!("dns:{subdomain}");
    let records = match state.cache.get(&dns_key).await {
        Ok(Some(json_str)) => serde_json::from_str::<Vec<DnsRecord>>(&json_str).unwrap_or_default(),
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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
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

        // Validate domain (security + RFC compliance)
        if let Err(e) = validate_dns_domain(&record.domain) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": e,
                    "code": "invalid_domain"
                })),
            )
                .into_response();
        }

        // Validate record value (e.g., valid IPv4 for A records)
        if let Err(e) = validate_dns_value(&record.r#type, &record.value) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": e,
                    "code": "invalid_value"
                })),
            )
                .into_response();
        }
    }

    // Clear old records
    delete_dns_record_entries(&state.cache, &subdomain).await;

    // Store new records
    let mut final_records = Vec::new();
    for record in records.records {
        // Use validated/normalized domain
        let short_domain = validate_dns_domain(&record.domain).unwrap(); // Already validated above

        // Compute full FQDN for DNS cache key
        let fqdn = format!("{}.{}.{}.", short_domain, subdomain, CONFIG.server_domain);

        let _ = state
            .cache
            .set(&format!("dns:{}:{}", record.r#type, fqdn), &record.value)
            .await;

        // Store the short domain for display, not the full FQDN
        final_records.push(json!({
            "domain": short_domain,
            "type": record.r#type,
            "value": record.value,
        }));
    }

    if let Err(e) = state
        .cache
        .set(
            &format!("dns:{subdomain}"),
            &serde_json::to_string(&final_records).unwrap_or_default(),
        )
        .await
    {
        return (
            StatusCode::INSUFFICIENT_STORAGE,
            Json(json!({"error": format!("Storage limit exceeded: {e}"), "code": "storage_error"})),
        )
            .into_response();
    }

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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    // Calculate total size of all files (raw content is base64 encoded)
    let total_size: usize = files
        .files
        .values()
        .map(|response| response.raw.len())
        .sum();

    let max_size = CONFIG.max_subdomain_size_bytes;
    if total_size > max_size {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "error": format!("Total file size exceeds limit of {} MB", max_size / 1024 / 1024),
                "code": "file_tree_too_large",
                "current_size": total_size,
                "max_size": max_size
            })),
        )
            .into_response();
    }

    if let Err(e) = state
        .cache
        .set(
            &format!("files:{subdomain}"),
            &serde_json::to_string(&files).unwrap_or_default(),
        )
        .await
    {
        return (
            StatusCode::INSUFFICIENT_STORAGE,
            Json(json!({"error": format!("Storage limit exceeded: {e}"), "code": "storage_error"})),
        )
            .into_response();
    }

    (StatusCode::OK, Json(json!({ "message": "Files updated" }))).into_response()
}

/// GET /api/v2/files/:path - Get single file
pub async fn get_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
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

/// Body of `PUT /api/v2/files/*path`. Give the content as text in `body`, or base64 in `raw`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutFileRequest {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub headers: Option<Vec<crate::models::Header>>,
    #[serde(default)]
    pub status_code: Option<u16>,
}

/// A path in a session's file tree, as the tree stores it: no leading slash.
fn file_key(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let valid = !path.is_empty()
        && path.len() <= 1024
        && !path.split('/').any(|part| part == ".." || part == ".")
        && !path.contains('\0');
    valid.then(|| path.to_string())
}

/// The Content-Type a new file gets when the request names none.
fn default_content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        _ => "text/plain; charset=utf-8",
    }
}

fn file_error(status: StatusCode, code: &str, error: impl Into<String>) -> Response {
    (status, Json(json!({ "error": error.into(), "code": code }))).into_response()
}

/// Serializes read-modify-write of file trees, so that two single-file updates sent at the
/// same time cannot drop one another. Held only for a cache read and write.
fn file_tree_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(Default::default)
}

async fn load_file_tree(cache: &Cache, subdomain: &str) -> FileTree {
    cache
        .get(&format!("files:{subdomain}"))
        .await
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<FileTree>(&json).ok())
        .unwrap_or(FileTree {
            files: HashMap::new(),
        })
}

async fn save_file_tree(
    cache: &Cache,
    subdomain: &str,
    files: &FileTree,
) -> Result<(), Box<Response>> {
    let total_size: usize = files.files.values().map(|file| file.raw.len()).sum();
    let max_size = CONFIG.max_subdomain_size_bytes;
    if total_size > max_size {
        return Err(Box::new(file_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_tree_too_large",
            format!(
                "Total file size exceeds limit of {} MB",
                max_size / 1024 / 1024
            ),
        )));
    }
    cache
        .set(
            &format!("files:{subdomain}"),
            &serde_json::to_string(files).unwrap_or_default(),
        )
        .await
        .map_err(|e| {
            Box::new(file_error(
                StatusCode::INSUFFICIENT_STORAGE,
                "storage_error",
                format!("Storage limit exceeded: {e}"),
            ))
        })
}

/// PUT /api/v2/files/*path - Create or replace one file of the response tree
///
/// `{"body": "hello"}` is enough: the status defaults to 200 and the headers to a
/// Content-Type guessed from the extension plus `Access-Control-Allow-Origin: *`.
pub async fn put_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };
    let request: PutFileRequest = match json_body(&headers, &body, false) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let Some(key) = file_key(&path) else {
        return file_error(StatusCode::BAD_REQUEST, "invalid_path", "Invalid file path");
    };

    let raw = match (request.body, request.raw) {
        (Some(_), Some(_)) => {
            return file_error(
                StatusCode::BAD_REQUEST,
                "invalid_body",
                "Give either `body` (text) or `raw` (base64), not both",
            )
        }
        (Some(text), None) => BASE64.encode(text),
        (None, Some(raw)) => {
            if BASE64.decode(&raw).is_err() {
                return file_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_raw",
                    "`raw` must be standard base64",
                );
            }
            raw
        }
        (None, None) => String::new(),
    };
    let status_code = request.status_code.unwrap_or(200);
    if !(100..=599).contains(&status_code) {
        return file_error(
            StatusCode::BAD_REQUEST,
            "invalid_status_code",
            "status_code must be between 100 and 599",
        );
    }
    let response_headers = request.headers.unwrap_or_else(|| {
        vec![
            crate::models::Header {
                header: "Access-Control-Allow-Origin".to_string(),
                value: "*".to_string(),
            },
            crate::models::Header {
                header: "Content-Type".to_string(),
                value: default_content_type(&key).to_string(),
            },
        ]
    });

    let _tree = file_tree_lock().lock().await;
    let mut files = load_file_tree(&state.cache, &subdomain).await;
    let file = crate::models::Response {
        raw,
        headers: response_headers,
        status_code,
    };
    let created = files.files.insert(key.clone(), file.clone()).is_none();
    if let Err(response) = save_file_tree(&state.cache, &subdomain, &files).await {
        return *response;
    }

    (
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(json!({
            "path": key,
            "url": format!(
                "{}/{key}",
                public_origin(&format!("{subdomain}.{}", CONFIG.server_domain))
            ),
            "file": file,
        })),
    )
        .into_response()
}

/// DELETE /api/v2/files/*path - Remove one file of the response tree
pub async fn delete_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };
    let Some(key) = file_key(&path) else {
        return file_error(StatusCode::BAD_REQUEST, "invalid_path", "Invalid file path");
    };

    let _tree = file_tree_lock().lock().await;
    let mut files = load_file_tree(&state.cache, &subdomain).await;
    if files.files.remove(&key).is_none() {
        return file_error(StatusCode::NOT_FOUND, "not_found", "File not found");
    }
    if let Err(response) = save_file_tree(&state.cache, &subdomain, &files).await {
        return *response;
    }
    (
        StatusCode::OK,
        Json(json!({ "path": key, "deleted": true })),
    )
        .into_response()
}

// ============================================================================
// Request Routes
// ============================================================================

/// GET /api/v2/requests - List requests
///
/// Query params:
/// - token: JWT token (or use Authorization header)
/// - limit: Max results (default 100, max 1000)
/// - offset: Skip first N results (default 0). Ignored with `after`.
/// - after: Only requests captured after the request with this ID, oldest first. Pass the
///   `cursor` of the previous response to get only what is new.
/// - wait: When nothing matches yet, hold the response up to this many seconds (max 60)
///   until a request arrives. With `after`, this is a long poll for the next request.
/// - type: Only `http`, `dns`, `smtp` or `tcp` requests
/// - last: Only the newest N matching requests (still oldest first)
/// - view: `summary` for one compact object per request instead of the full capture
pub async fn list_requests(
    State(state): State<AppState>,
    Query(query): Query<PaginatedTokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let kind = match query.r#type.as_deref().map(str::to_lowercase) {
        None => None,
        Some(kind) if ["http", "dns", "smtp", "tcp"].contains(&kind.as_str()) => Some(kind),
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Unknown type '{other}': use http, dns, smtp or tcp"),
                    "code": "invalid_type"
                })),
            )
                .into_response()
        }
    };
    let summary = match query.view.as_deref() {
        None | Some("full") => false,
        Some("summary") => true,
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Unknown view '{other}': use full or summary"),
                    "code": "invalid_view"
                })),
            )
                .into_response()
        }
    };
    let last = query.last.map(|n| n.clamp(1, MAX_LIMIT));
    let mut wait = Duration::from_secs(query.wait.unwrap_or(0).min(MAX_WAIT_SECS));
    let _slot = if wait.is_zero() {
        None
    } else {
        let slot = LongPollSlot::acquire(&subdomain);
        if slot.is_none() {
            wait = Duration::ZERO;
        }
        slot
    };
    // Subscribe before anything is read, so a request that arrives in between is not missed.
    let mut updates = state.tx.subscribe();

    // `after=now` means after the newest request stored at this moment. On an empty session
    // everything that arrives is new, so there is nothing to skip.
    let resolved_after = match query.after.as_deref() {
        Some("now") => newest_id(&state.cache, &subdomain).await,
        other => other.map(str::to_string),
    };
    let filter = PageFilter {
        after: resolved_after.as_deref(),
        kind: kind.as_deref(),
        limit,
        offset,
        last,
    };
    let deadline = Instant::now() + wait;

    let page = loop {
        let page = match fetch_page(&state.cache, &subdomain, &filter).await {
            Some(page) => page,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": "No request has the ID given in `after`: it was deleted or \
                                  evicted. List without `after` to start over.",
                        "code": "cursor_not_found"
                    })),
                )
                    .into_response()
            }
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if !page.requests.is_empty() || remaining.is_zero() {
            break page;
        }

        // Nothing yet: wait for this session's next request, or for the deadline.
        let arrived = tokio::time::timeout(remaining, async {
            loop {
                match updates.recv().await {
                    // A new request, or the session's suspension, which must end the wait
                    Ok(msg)
                        if msg.subdomain == subdomain
                            && (msg.cmd == "new_request" || msg.cmd == "suspended") =>
                    {
                        return true
                    }
                    Ok(_) => {}
                    // Missed some messages: one of them may have been ours, so look.
                    Err(broadcast::error::RecvError::Lagged(_)) => return true,
                    Err(broadcast::error::RecvError::Closed) => return false,
                }
            }
        })
        .await;
        // The session may have been suspended while it waited: what arrives from then on is
        // for the abuse admin only.
        if state.abuse.is_blocked(&subdomain) {
            return suspended_rejection();
        }
        if arrived != Ok(true) {
            // Timed out: answer with what there is, which is nothing new.
            break page;
        }
    };

    // The cursor to pass as `after` next time: the newest request returned, or the cursor
    // that was in effect when nothing new arrived (`now` stays `now` on an empty session).
    let cursor = page
        .requests
        .last()
        .and_then(|r| r.get("_id"))
        .and_then(|id| id.as_str())
        .map(str::to_string)
        .or(resolved_after)
        .or_else(|| (query.after.as_deref() == Some("now")).then(|| "now".to_string()));
    let requests: Vec<serde_json::Value> = if summary {
        page.requests.iter().map(summarize).collect()
    } else {
        page.requests
    };
    let mut pagination = json!({
        "total": page.total,
        "limit": limit,
        "has_more": page.has_more
    });
    if query.after.is_none() && last.is_none() {
        pagination["offset"] = json!(offset);
    }

    (
        StatusCode::OK,
        Json(json!({
            "requests": requests,
            "cursor": cursor,
            "pagination": pagination,
        })),
    )
        .into_response()
}

struct PageFilter<'a> {
    after: Option<&'a str>,
    kind: Option<&'a str>,
    limit: usize,
    offset: usize,
    last: Option<usize>,
}

/// ID of the newest stored request of a session, if it has any.
async fn newest_id(cache: &Cache, subdomain: &str) -> Option<String> {
    all_requests(cache, subdomain)
        .await
        .iter()
        .rev()
        .find_map(|r| r.get("_id").and_then(|id| id.as_str()).map(str::to_string))
}

#[cfg(test)]
pub(crate) fn summarize_for_tests(request: &serde_json::Value) -> serde_json::Value {
    summarize(request)
}

/// Characters of a body shown in the summary view.
const SUMMARY_BODY_CHARS: usize = 1000;

/// One compact object for a captured request: what it was in a line, and its body as text.
/// The full capture stays one `GET /requests/{id}` away.
fn summarize(request: &serde_json::Value) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    let text = |key: &str| request.get(key).and_then(|v| v.as_str()).unwrap_or("");
    let kind = text("type");
    let line = match kind {
        "http" => format!("{} {}{}", text("method"), text("path"), text("query")),
        // Resolvers often randomize the case of names they query: show them lowercased
        "dns" => format!(
            "{} {}",
            text("query_type"),
            text("domain").trim_end_matches('.').to_lowercase()
        ),
        "smtp" => format!(
            "mail from {} to {}: {}",
            text("from"),
            text("to"),
            text("subject")
        ),
        "tcp" => format!(
            "TCP connection to port {}",
            request.get("port").cloned().unwrap_or_default()
        ),
        other => other.to_string(),
    };

    let mut out = json!({
        "_id": request.get("_id"),
        "type": kind,
        "date": request.get("date"),
        "ip": request.get("ip"),
        "summary": line,
    });
    if let Some(country) = request.get("country").filter(|c| !c.is_null()) {
        out["country"] = country.clone();
    }

    let body = match kind {
        "http" => BASE64.decode(text("raw")).ok(),
        "smtp" => Some(text("text_body").as_bytes().to_vec()),
        _ => None,
    };
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        match String::from_utf8(body) {
            Ok(body) => {
                let shown: String = body.chars().take(SUMMARY_BODY_CHARS).collect();
                if shown.len() < body.len() {
                    out["body_truncated"] = json!(true);
                }
                out["body_text"] = json!(shown);
            }
            Err(e) => {
                out["body_text"] = json!(format!("<{} bytes of binary data>", e.as_bytes().len()))
            }
        }
    }
    out
}

struct RequestPage {
    /// Matching requests in total, before `limit` (and `offset`) apply
    total: usize,
    has_more: bool,
    requests: Vec<serde_json::Value>,
}

/// One page of a session's requests. `None` if `after` names a request that is not stored.
async fn fetch_page(
    cache: &Cache,
    subdomain: &str,
    filter: &PageFilter<'_>,
) -> Option<RequestPage> {
    // The common case, a plain page, reads only that page.
    if filter.after.is_none() && filter.kind.is_none() && filter.last.is_none() {
        let (total, requests) =
            paginated_requests(cache, subdomain, filter.limit, filter.offset).await;
        return Some(RequestPage {
            total,
            has_more: filter.offset.saturating_add(filter.limit) < total,
            requests,
        });
    }

    let all = all_requests(cache, subdomain).await;
    // With a cursor: everything after it. Without: everything, paged by `offset` below.
    let start = match filter.after {
        Some(after) => {
            all.iter()
                .position(|r| r.get("_id").and_then(|id| id.as_str()) == Some(after))?
                + 1
        }
        None => 0,
    };
    let matching: Vec<_> = all
        .into_iter()
        .skip(start)
        .filter(|r| filter.kind.is_none_or(|kind| is_type(r, kind)))
        .collect();
    let total = matching.len();
    if let Some(last) = filter.last {
        // The newest `last`, oldest first; `has_more` means older ones exist
        let skip = total.saturating_sub(last);
        return Some(RequestPage {
            total,
            has_more: skip > 0,
            requests: matching.into_iter().skip(skip).collect(),
        });
    }
    let offset = if filter.after.is_some() {
        0
    } else {
        filter.offset
    };
    Some(RequestPage {
        total,
        has_more: offset.saturating_add(filter.limit) < total,
        requests: matching
            .into_iter()
            .skip(offset)
            .take(filter.limit)
            .collect(),
    })
}

/// Every stored, non-deleted request of a session, oldest first.
async fn all_requests(cache: &Cache, subdomain: &str) -> Vec<serde_json::Value> {
    cache
        .lrange(&format!("requests:{subdomain}"), 0, -1)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .filter(|v| !v.as_object().map(|o| o.is_empty()).unwrap_or(true))
        .collect()
}

fn is_type(request: &serde_json::Value, kind: &str) -> bool {
    request.get("type").and_then(|t| t.as_str()) == Some(kind)
}

/// GET /api/v2/requests/:id - Get single request
pub async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    // Find request by scanning the list (indices are not stable due to eviction)
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

    let mut found_index: Option<usize> = None;
    for (i, req_str) in requests.iter().enumerate() {
        if let Ok(req) = serde_json::from_str::<serde_json::Value>(req_str) {
            if req.get("_id").and_then(|v| v.as_str()) == Some(&id) {
                found_index = Some(i);
                break;
            }
        }
    }

    let index = match found_index {
        Some(i) => i,
        None => {
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

    // Set to empty object to mark as deleted
    let _ = state
        .cache
        .lset(&format!("requests:{subdomain}"), index as isize, "{}")
        .await;

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
    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    // Delete the requests list
    let _ = state.cache.delete(&format!("requests:{subdomain}")).await;

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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();

    // Check rate limit (reuses session creation rate limiter)
    if let Err(response) = check_share_rate_limit(&state, &client_ip).await {
        return response.into_response();
    }

    let subdomain = match verify_token_or_error(query.token.as_deref(), &headers, &state.abuse) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
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

    // Links to a blocked subdomain's requests stop working with its token
    if state.abuse.is_blocked(&claims.subdomain) {
        return suspended_rejection();
    }

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_unblocked_skips_blocked_labels() {
        let draws = ["11111111", "22222222", "33333333", "44444444"];
        let mut next = draws.iter();
        let picked = pick_unblocked(
            || next.next().unwrap().to_string(),
            |subdomain| subdomain != "44444444",
        );
        assert_eq!(picked.as_deref(), Some("44444444"));

        // When every draw is blocked it gives up instead of drawing forever
        assert_eq!(pick_unblocked(|| "11111111".to_string(), |_| true), None);
    }

    #[test]
    fn test_validate_dns_domain_valid() {
        // Wildcard
        assert_eq!(validate_dns_domain("*").unwrap(), "*");

        // Simple subdomains
        assert_eq!(validate_dns_domain("www").unwrap(), "www");
        assert_eq!(validate_dns_domain("api").unwrap(), "api");

        // Nested subdomains
        assert_eq!(validate_dns_domain("a.b.c").unwrap(), "a.b.c");
        assert_eq!(validate_dns_domain("a.b.c.d.e").unwrap(), "a.b.c.d.e");

        // With underscores (for ACME challenges)
        assert_eq!(
            validate_dns_domain("_acme-challenge").unwrap(),
            "_acme-challenge"
        );
        assert_eq!(
            validate_dns_domain("_acme-challenge.www").unwrap(),
            "_acme-challenge.www"
        );

        // With hyphens
        assert_eq!(validate_dns_domain("my-api").unwrap(), "my-api");

        // Normalization: trim whitespace
        assert_eq!(validate_dns_domain("  www  ").unwrap(), "www");

        // Normalization: remove leading/trailing dots
        assert_eq!(validate_dns_domain(".www.").unwrap(), "www");

        // Normalization: lowercase
        assert_eq!(validate_dns_domain("WWW").unwrap(), "www");
    }

    #[test]
    fn test_validate_dns_domain_blocked() {
        // Empty domain
        assert!(validate_dns_domain("").is_err());
        assert!(validate_dns_domain("   ").is_err());
        assert!(validate_dns_domain("..").is_err());

        // Root subdomain (@) - security risk
        assert!(validate_dns_domain("@").is_err());
        let err = validate_dns_domain("@").unwrap_err();
        assert!(err.contains("security"));

        // Invalid characters
        assert!(validate_dns_domain("test@domain").is_err());
        assert!(validate_dns_domain("test/path").is_err());
        assert!(validate_dns_domain("test:8080").is_err());

        // Empty label in middle
        assert!(validate_dns_domain("a..b").is_err());
    }

    #[test]
    fn test_validate_dns_domain_length_limits() {
        // Label > 63 chars should fail
        let long_label = "a".repeat(64);
        assert!(validate_dns_domain(&long_label).is_err());

        // Label == 63 chars should pass
        let max_label = "a".repeat(63);
        assert!(validate_dns_domain(&max_label).is_ok());

        // Total > 253 chars should fail
        let long_domain = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63)
        );
        assert!(validate_dns_domain(&long_domain).is_err());
    }

    #[test]
    fn test_validate_dns_value_a_record() {
        // Valid IPv4
        assert!(validate_dns_value("A", "1.2.3.4").is_ok());
        assert!(validate_dns_value("A", "192.168.1.1").is_ok());
        assert!(validate_dns_value("A", "255.255.255.255").is_ok());

        // Multiple IPs separated by %
        assert!(validate_dns_value("A", "1.2.3.4%5.6.7.8").is_ok());

        // Invalid IPv4
        assert!(validate_dns_value("A", "not-an-ip").is_err());
        assert!(validate_dns_value("A", "256.1.1.1").is_err());
        assert!(validate_dns_value("A", "1.2.3").is_err());
        assert!(validate_dns_value("A", "::1").is_err()); // IPv6 not allowed for A records
    }

    #[test]
    fn test_validate_dns_value_aaaa_record() {
        // Valid IPv6
        assert!(validate_dns_value("AAAA", "::1").is_ok());
        assert!(validate_dns_value("AAAA", "2001:db8::1").is_ok());
        assert!(validate_dns_value("AAAA", "fe80::1").is_ok());

        // Multiple IPs separated by %
        assert!(validate_dns_value("AAAA", "::1%::2").is_ok());

        // Invalid IPv6
        assert!(validate_dns_value("AAAA", "not-an-ip").is_err());
        assert!(validate_dns_value("AAAA", "1.2.3.4").is_err()); // IPv4 not allowed for AAAA records
    }

    #[test]
    fn test_validate_dns_value_cname_txt() {
        // Valid CNAME
        assert!(validate_dns_value("CNAME", "example.com").is_ok());
        assert!(validate_dns_value("CNAME", "example.com.").is_ok());

        // Valid TXT
        assert!(validate_dns_value("TXT", "v=spf1 include:example.com ~all").is_ok());
        assert!(validate_dns_value("TXT", "verification=abc123").is_ok());

        // Empty values should fail
        assert!(validate_dns_value("CNAME", "").is_err());
        assert!(validate_dns_value("TXT", "").is_err());
    }
}
