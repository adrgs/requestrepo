//! HTTP endpoints of the subdomain blocklist.
//!
//! None of these routes are CORS-enabled, and reports must be `application/json`, which a
//! cross-site form or beacon cannot send without a CORS preflight: no website can make its
//! visitors' browsers file reports.

use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::warn;

use super::limiter::network_label;
use super::{
    AdminError, Applied, Category, Entry, MutateError, Report, Source, REPORT_RATE_LIMIT,
    REPORT_RATE_WINDOW,
};
use crate::http::routes_v2::{delete_dns_record_entries, json_body, paginated_requests};
use crate::http::AppState;
use crate::models::CacheMessage;
use crate::utils::auth::bearer_token;
use crate::utils::config::CONFIG;
use crate::utils::{get_current_timestamp, verify_subdomain};

/// Largest accepted report or admin body. Both hold a few short text fields.
const MAX_BODY_BYTES: usize = 4 * 1024;
const MAX_SUBDOMAIN_INPUT: usize = 512;
/// Reasons are short on purpose: they are a note for the admin, not a place to store data.
pub const MAX_REASON_CHARS: usize = 300;

const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 500;

/// All blocklist routes. The policy and report routes are public; the admin routes need
/// `ABUSE_ADMIN_TOKEN` as a Bearer token.
pub fn router() -> Router<AppState> {
    let admin = "/api/v2/abuse/admin/subdomains";
    Router::new()
        .route("/api/v2/abuse", get(policy))
        .route(
            "/api/v2/abuse/reports",
            post(create_report).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route(admin, get(list_entries))
        .route(
            &format!("{admin}/:subdomain"),
            get(get_entry)
                .put(suspend_entry)
                .delete(lift_entry)
                .layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route(
            &format!("{admin}/:subdomain/requests"),
            get(list_entry_requests),
        )
        .route(&format!("{admin}/:subdomain/sources"), get(list_sources))
        .route(&format!("{admin}/:subdomain/data"), delete(purge_data))
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReportRequest {
    pub subdomain: String,
    pub category: Category,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuspendRequest {
    #[serde(default)]
    pub category: Option<Category>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// An entry without its reports, for list responses.
#[derive(Debug, Serialize)]
struct EntrySummary<'a> {
    subdomain: &'a str,
    category: Category,
    source: Source,
    suspended_at: i64,
    updated_at: i64,
    report_count: u32,
}

impl<'a> From<&'a Entry> for EntrySummary<'a> {
    fn from(entry: &'a Entry) -> Self {
        Self {
            subdomain: &entry.subdomain,
            category: entry.category,
            source: entry.source,
            suspended_at: entry.suspended_at,
            updated_at: entry.updated_at,
            report_count: entry.report_count,
        }
    }
}

// ============================================================================
// Public
// ============================================================================

/// GET /api/v2/abuse - How reporting works on this instance
pub async fn policy(State(state): State<AppState>) -> Response {
    Json(json!({
        "reporting_enabled": state.abuse.settings().reporting_enabled,
        "effect": "A report suspends the subdomain immediately.",
        "categories": Category::ALL,
        "rate_limit": {
            "reports": REPORT_RATE_LIMIT,
            "window_secs": REPORT_RATE_WINDOW.as_secs(),
            "per": "IPv4 address or IPv6 /48",
        },
        "report_endpoint": "/api/v2/abuse/reports",
        "report_page": "/abuse",
        "contact": CONFIG.abuse_contact,
    }))
    .into_response()
}

/// POST /api/v2/abuse/reports - Report a subdomain for abuse, which suspends it at once
pub async fn create_report(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.abuse.settings().reporting_enabled {
        return error(
            StatusCode::NOT_FOUND,
            "reporting_disabled",
            "Abuse reporting is disabled on this instance",
        );
    }
    let body: CreateReportRequest = match json_body(&headers, &body, true) {
        Ok(body) => body,
        Err(response) => return *response,
    };

    // Validation comes first, so that a typo does not cost the reporter an attempt.
    let Some(subdomain) = normalize_subdomain(&body.subdomain) else {
        return invalid_subdomain();
    };
    let Ok(reason) = optional(body.reason.as_deref(), MAX_REASON_CHARS) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_reason",
            format!("The reason must be at most {MAX_REASON_CHARS} characters"),
        );
    };

    let ip = addr.ip();
    if let Err(retry_after) = state.abuse.check_report_rate(ip) {
        return rate_limited(retry_after);
    }

    let report = Report {
        at: get_current_timestamp(),
        category: body.category,
        network: network_label(ip),
        reason,
    };
    match state.abuse.report(&subdomain, report).await {
        Ok(Applied { value: newly, .. }) => {
            if newly {
                warn!("Subdomain {subdomain} suspended by an abuse report");
                if let Some(entry) = state.abuse.suspension(&subdomain) {
                    announce_suspension(&state.tx, &entry);
                }
            }
            (
                if newly {
                    StatusCode::CREATED
                } else {
                    StatusCode::OK
                },
                Json(json!({
                    "subdomain": subdomain,
                    "suspended": true,
                    "already_suspended": !newly,
                    "message": "The subdomain is suspended.",
                })),
            )
                .into_response()
        }
        Err(MutateError::Full) => error(
            StatusCode::INSUFFICIENT_STORAGE,
            "blocklist_full",
            "The blocklist is full",
        ),
    }
}

fn rate_limited(retry_after: Duration) -> Response {
    let retry_after = retry_after.as_secs().max(1);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, retry_after.to_string())],
        Json(json!({
            "error": "Too many reports from your address. Try again later.",
            "code": "rate_limited",
            "retry_after": retry_after,
        })),
    )
        .into_response()
}

/// Parses what a reporter typed for the subdomain: the label (`abc12345`), a hostname
/// (`abc12345.example.com`, with or without a port) or a URL. Only subdomains of this
/// instance are accepted.
fn normalize_subdomain(input: &str) -> Option<String> {
    let input = input.trim().to_lowercase();
    if input.is_empty() || input.len() > MAX_SUBDOMAIN_INPUT {
        return None;
    }

    // For a URL keep only the host, which never includes the port.
    let host = if input.contains("://") {
        Uri::from_str(&input).ok()?.host()?.to_string()
    } else {
        input
    };
    // A hostname with a port but no scheme.
    let host = match host.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            host.to_string()
        }
        _ => host,
    };
    let host = host.trim_end_matches('.');

    // A name under the subdomain (`x.abc12345.example.com`) belongs to it too.
    let suffix = format!(".{}", CONFIG.server_domain);
    let label = match host.strip_suffix(&suffix) {
        Some(rest) => rest.rsplit('.').next().unwrap_or(rest),
        None => host,
    };
    verify_subdomain(
        label,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    )
    .then(|| label.to_string())
}

/// Trims `value` and enforces a character limit. A blank value means it was left out.
fn optional(value: Option<&str>, max_chars: usize) -> Result<Option<String>, ()> {
    let Some(value) = value.map(str::trim) else {
        return Ok(None);
    };
    if value.chars().count() > max_chars {
        return Err(());
    }
    Ok((!value.is_empty()).then(|| value.to_string()))
}

// ============================================================================
// Admin
// ============================================================================

/// Requires `Authorization: Bearer <ABUSE_ADMIN_TOKEN>`. Header only: no cookie, so no
/// other site can ride on an admin's browser.
fn require_admin(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    state
        .abuse
        .verify_admin(bearer_token(headers).as_deref())
        .err()
        .map(|err| match err {
            AdminError::NotConfigured => error(
                StatusCode::FORBIDDEN,
                "admin_not_configured",
                "The abuse admin API is off: set ABUSE_ADMIN_TOKEN to turn it on",
            ),
            AdminError::Invalid => error(
                StatusCode::UNAUTHORIZED,
                "invalid_admin_token",
                "Send the abuse admin token as Authorization: Bearer <token>",
            ),
        })
}

/// Authorizes an admin request and parses the subdomain in its path, which may be given as
/// a label, hostname or URL.
fn admin_subdomain(
    state: &AppState,
    headers: &HeaderMap,
    subdomain: &str,
) -> Result<String, Box<Response>> {
    if let Some(response) = require_admin(state, headers) {
        return Err(Box::new(response));
    }
    normalize_subdomain(subdomain).ok_or_else(|| Box::new(invalid_subdomain()))
}

/// GET /api/v2/abuse/admin/subdomains - Suspended subdomains, most recently changed first
pub async fn list_entries(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Response {
    if let Some(response) = require_admin(&state, &headers) {
        return response;
    }

    let (limit, offset) = page(query.limit, query.offset);
    let search = query
        .q
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_lowercase();

    let all = state.abuse.entries();
    let mut entries: Vec<&Entry> = all
        .iter()
        .map(Arc::as_ref)
        .filter(|entry| entry.subdomain.contains(&search))
        .collect();
    entries.sort_unstable_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.subdomain.cmp(&b.subdomain))
    });

    let total = entries.len();
    let page: Vec<EntrySummary> = entries
        .iter()
        .skip(offset)
        .take(limit)
        .map(|entry| EntrySummary::from(*entry))
        .collect();

    Json(json!({
        "subdomains": page,
        "pagination": pagination(total, limit, offset),
    }))
    .into_response()
}

/// GET /api/v2/abuse/admin/subdomains/:subdomain - One suspension, with its recent reports
pub async fn get_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match admin_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };
    match state.abuse.suspension(&subdomain) {
        Some(entry) => {
            let captured = state
                .cache
                .llen(&format!("requests:{subdomain}"))
                .await
                .unwrap_or(0);
            let mut body = serde_json::to_value(entry.as_ref()).unwrap_or_default();
            body["captured_requests"] = json!(captured);
            Json(body).into_response()
        }
        None => not_suspended(),
    }
}

/// PUT /api/v2/abuse/admin/subdomains/:subdomain - Suspend a subdomain. The body is
/// optional: `{"category"?, "note"?}`. On a suspended subdomain it updates the reason.
pub async fn suspend_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let subdomain = match admin_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };
    let request: SuspendRequest = if body.iter().all(u8::is_ascii_whitespace) {
        SuspendRequest::default()
    } else {
        match json_body(&headers, &body, false) {
            Ok(request) => request,
            Err(response) => return *response,
        }
    };
    let Ok(note) = optional(request.note.as_deref(), MAX_REASON_CHARS) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_note",
            format!("The note must be at most {MAX_REASON_CHARS} characters"),
        );
    };

    match state
        .abuse
        .suspend(&subdomain, request.category, note)
        .await
    {
        Ok(applied) => {
            let entry = state.abuse.suspension(&subdomain);
            if applied.value {
                if let Some(entry) = &entry {
                    announce_suspension(&state.tx, entry);
                }
            }
            (
                if applied.value {
                    StatusCode::CREATED
                } else {
                    StatusCode::OK
                },
                Json(json!({
                    "subdomain": subdomain,
                    "suspended": true,
                    "already_suspended": !applied.value,
                    "persisted": applied.persisted,
                    "entry": entry.as_deref(),
                })),
            )
                .into_response()
        }
        Err(MutateError::Full) => error(
            StatusCode::INSUFFICIENT_STORAGE,
            "blocklist_full",
            "The blocklist is full",
        ),
    }
}

/// DELETE /api/v2/abuse/admin/subdomains/:subdomain - Lift the suspension
pub async fn lift_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match admin_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };
    match state.abuse.lift(&subdomain).await {
        Applied { value: false, .. } => not_suspended(),
        Applied { persisted, .. } => Json(json!({
            "subdomain": subdomain,
            "suspended": false,
            "persisted": persisted,
        }))
        .into_response(),
    }
}

/// DELETE /api/v2/abuse/admin/subdomains/:subdomain/data - Delete the subdomain's files,
/// DNS records and captured requests
pub async fn purge_data(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match admin_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    let cache = &state.cache;
    let _ = cache.delete(&format!("files:{subdomain}")).await;
    delete_dns_record_entries(cache, &subdomain).await;
    let _ = cache.delete(&format!("dns:{subdomain}")).await;
    let _ = cache.delete(&format!("requests:{subdomain}")).await;

    // The subdomain may not be suspended, with an owner's dashboard still showing them.
    let _ = state.tx.send(CacheMessage {
        cmd: "delete_all".to_string(),
        subdomain: subdomain.clone(),
        data: "{}".to_string(),
    });

    Json(json!({
        "subdomain": subdomain,
        "purged": ["files", "dns", "requests"],
    }))
    .into_response()
}

/// GET /api/v2/abuse/admin/subdomains/:subdomain/requests - What a suspended subdomain
/// captured, including traffic that arrived while it was suspended. Read-only.
pub async fn list_entry_requests(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match suspended_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    let (limit, offset) = page(query.limit, query.offset);
    let (total, requests) = paginated_requests(&state.cache, &subdomain, limit, offset).await;

    Json(json!({
        "requests": requests,
        "pagination": pagination(total, limit, offset),
    }))
    .into_response()
}

#[derive(Debug, Serialize)]
struct SourceAddress {
    ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    requests: usize,
    first_seen: i64,
    last_seen: i64,
    types: Vec<String>,
}

/// GET /api/v2/abuse/admin/subdomains/:subdomain/sources - Every source address that
/// contacted a suspended subdomain, most active first. For a botnet beacon these are its
/// victims, in the form a CERT or abuse.ch can use to notify them.
pub async fn list_sources(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match suspended_subdomain(&state, &headers, &subdomain) {
        Ok(subdomain) => subdomain,
        Err(response) => return *response,
    };

    let requests = state
        .cache
        .lrange(&format!("requests:{subdomain}"), 0, -1)
        .await
        .unwrap_or_default();

    let mut sources: HashMap<String, SourceAddress> = HashMap::new();
    for raw in &requests {
        let Ok(request) = serde_json::from_str::<serde_json::Value>(raw) else {
            continue;
        };
        let Some(ip) = request.get("ip").and_then(|ip| ip.as_str()) else {
            continue;
        };
        let date = request.get("date").and_then(|d| d.as_i64()).unwrap_or(0);
        let kind = request
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");
        let source = sources
            .entry(ip.to_string())
            .or_insert_with(|| SourceAddress {
                ip: ip.to_string(),
                country: request
                    .get("country")
                    .and_then(|c| c.as_str())
                    .map(str::to_string),
                requests: 0,
                first_seen: date,
                last_seen: date,
                types: Vec::new(),
            });
        source.requests += 1;
        source.first_seen = source.first_seen.min(date);
        source.last_seen = source.last_seen.max(date);
        if !source.types.iter().any(|t| t == kind) {
            source.types.push(kind.to_string());
        }
    }

    let mut sources: Vec<SourceAddress> = sources.into_values().collect();
    sources.sort_unstable_by(|a, b| b.requests.cmp(&a.requests).then(a.ip.cmp(&b.ip)));

    Json(json!({
        "subdomain": subdomain,
        "total_requests": requests.len(),
        "unique_sources": sources.len(),
        "sources": sources,
    }))
    .into_response()
}

/// The subdomain in an admin path, if it is suspended. Captured traffic is only open to the
/// admin for subdomains that were suspended.
fn suspended_subdomain(
    state: &AppState,
    headers: &HeaderMap,
    subdomain: &str,
) -> Result<String, Box<Response>> {
    let subdomain = admin_subdomain(state, headers, subdomain)?;
    if state.abuse.is_blocked(&subdomain) {
        Ok(subdomain)
    } else {
        Err(Box::new(not_suspended()))
    }
}

fn page(limit: Option<usize>, offset: Option<usize>) -> (usize, usize) {
    (
        limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT),
        offset.unwrap_or(0),
    )
}

fn pagination(total: usize, limit: usize, offset: usize) -> serde_json::Value {
    json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "has_more": offset.saturating_add(limit) < total,
    })
}

// ============================================================================
// Shared Responses
// ============================================================================

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "error": message.into(), "code": code })),
    )
        .into_response()
}

fn invalid_subdomain() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "invalid_subdomain",
        format!(
            "Give a subdomain of {}: a label, a hostname or a URL",
            CONFIG.server_domain
        ),
    )
}

fn not_suspended() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "not_suspended",
        "The subdomain is not suspended",
    )
}

/// Sent to the owner of a suspended subdomain on any endpoint that accepts its token.
pub(crate) fn suspended_rejection(entry: &Entry) -> Response {
    let mut body = json!({
        "error": "This subdomain was suspended after an abuse report. Create a new session.",
        "code": "subdomain_suspended",
        "category": entry.category,
    });
    if let Some(contact) = &CONFIG.abuse_contact {
        body["contact"] = json!(contact);
    }
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

/// Sent for any traffic to a suspended subdomain: a small page for browsers, plain text for
/// everything else. Nothing of the subdomain's own content is served, and it is not cached.
pub(crate) fn suspended_traffic_response(accept: Option<&str>) -> Response {
    let contact = CONFIG.abuse_contact.as_deref();
    let wants_html = accept.is_some_and(|accept| accept.contains("text/html"));

    let (content_type, body) = if wants_html {
        let contact = contact
            .map(|contact| {
                format!(
                    "<p>If you believe this is a mistake, contact {}.</p>",
                    html_escape(contact)
                )
            })
            .unwrap_or_default();
        (
            "text/html; charset=utf-8",
            format!(
                "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
                 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
                 <meta name=\"robots\" content=\"noindex\">\
                 <title>Suspended</title><style>\
                 body{{font:16px/1.5 system-ui,sans-serif;margin:0;min-height:100vh;\
                 display:grid;place-items:center;background:#fafafa;color:#18181b}}\
                 main{{max-width:28rem;padding:2rem}}h1{{font-size:1.25rem;margin:0 0 .5rem}}\
                 p{{margin:.5rem 0;color:#52525b}}\
                 @media(prefers-color-scheme:dark){{body{{background:#18181b;color:#fafafa}}\
                 p{{color:#a1a1aa}}}}</style></head><body><main>\
                 <h1>This subdomain has been suspended</h1>\
                 <p>It was reported for abuse and no longer serves content.</p>{contact}\
                 </main></body></html>"
            ),
        )
    } else {
        let mut body = String::from("410 Gone: this subdomain has been suspended for abuse.\n");
        if let Some(contact) = contact {
            body.push_str(&format!(
                "If you believe this is a mistake, contact {contact}.\n"
            ));
        }
        ("text/plain; charset=utf-8", body)
    };

    Response::builder()
        .status(StatusCode::GONE)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::VARY, "Accept")
        .header("x-robots-tag", "noindex")
        .body(Body::from(body))
        .unwrap()
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Tells the owner's live dashboards that the subdomain is now suspended, and why.
fn announce_suspension(tx: &broadcast::Sender<CacheMessage>, entry: &Entry) {
    let _ = tx.send(CacheMessage {
        cmd: "suspended".to_string(),
        subdomain: entry.subdomain.clone(),
        data: json!({ "category": entry.category }).to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label() -> String {
        crate::utils::get_random_subdomain()
    }

    #[test]
    fn subdomain_accepts_label_hostname_and_url() {
        let label = label();
        let domain = &CONFIG.server_domain;
        let expected = Some(label.clone());

        assert_eq!(normalize_subdomain(&label), expected);
        assert_eq!(normalize_subdomain(&format!("{label}.{domain}")), expected);
        assert_eq!(
            normalize_subdomain(&format!("{label}.{domain}:8080")),
            expected
        );
        assert_eq!(normalize_subdomain(&format!("{label}.{domain}.")), expected);
        assert_eq!(
            normalize_subdomain(&format!("http://{label}.{domain}/beacon?x=1")),
            expected
        );
        assert_eq!(
            normalize_subdomain(&format!("  HTTPS://{label}.{domain}:8443/x ")),
            expected
        );
        assert_eq!(
            normalize_subdomain(&format!("http://user:pw@{label}.{domain}/")),
            expected
        );
        // A name under the subdomain belongs to it
        assert_eq!(
            normalize_subdomain(&format!("data.x.{label}.{domain}")),
            expected
        );
        // Case never matters
        assert_eq!(
            normalize_subdomain(&format!(
                "{}.{}",
                label.to_uppercase(),
                domain.to_uppercase()
            )),
            expected
        );
    }

    #[test]
    fn subdomain_refuses_anything_else() {
        let label = label();

        assert_eq!(normalize_subdomain(""), None);
        assert_eq!(normalize_subdomain("   "), None);
        assert_eq!(
            normalize_subdomain(&"a".repeat(MAX_SUBDOMAIN_INPUT + 1)),
            None
        );
        // Not a label of this instance
        assert_eq!(normalize_subdomain(&format!("{label}.example.org")), None);
        assert_eq!(normalize_subdomain(&CONFIG.server_domain.clone()), None);
        assert_eq!(
            normalize_subdomain(&format!("x.{}", CONFIG.server_domain)),
            None
        );
        assert_eq!(normalize_subdomain(&format!("{}x", label)), None);
        // Without a scheme the whole input is taken as a hostname
        assert_eq!(
            normalize_subdomain(&format!("{label}.{}/path", CONFIG.server_domain)),
            None
        );
        assert_eq!(normalize_subdomain("http://"), None);
    }

    #[test]
    fn optional_trims_bounds_and_treats_blank_as_absent() {
        assert_eq!(optional(None, 20), Ok(None));
        assert_eq!(optional(Some("   "), 20), Ok(None));
        assert_eq!(optional(Some("  spaced  "), 20), Ok(Some("spaced".into())));
        assert_eq!(optional(Some("abcd"), 3), Err(()));
        // Limits count characters, not bytes
        assert_eq!(optional(Some("ééé"), 3), Ok(Some("ééé".into())));
    }

    #[test]
    fn the_suspended_page_escapes_the_contact() {
        let response = suspended_traffic_response(Some("text/html"));
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(html_escape("<a&\">"), "&lt;a&amp;&quot;&gt;");
    }
}
