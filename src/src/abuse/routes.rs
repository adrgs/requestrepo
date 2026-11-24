//! HTTP endpoints of the subdomain blocklist.
//!
//! None of these routes are CORS-enabled. The dashboard is same-origin, and a public write
//! endpoint must not be reachable from pages on other sites.

use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use super::limiter::network_label;
use super::{AdminError, Applied, Category, Entry, MutateError, Report, ReportOutcome, Source};
use crate::http::routes_v2::{delete_dns_record_entries, paginated_requests};
use crate::http::AppState;
use crate::models::CacheMessage;
use crate::utils::auth::extract_admin_token;
use crate::utils::config::CONFIG;
use crate::utils::{get_current_timestamp, verify_subdomain};

/// Largest accepted report or admin body. Both hold a few short text fields.
const MAX_BODY_BYTES: usize = 16 * 1024;
const MAX_SUBDOMAIN_INPUT: usize = 512;
const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_URL_CHARS: usize = 500;
const MAX_CONTACT_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 500;

const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 500;

/// All blocklist routes. The report route is public; the admin routes need the admin token.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/abuse/reports",
            post(create_report).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route("/api/v2/abuse/admin/entries", get(list_entries))
        .route(
            "/api/v2/abuse/admin/entries/:subdomain",
            get(get_entry).delete(lift_entry),
        )
        .route(
            "/api/v2/abuse/admin/entries/:subdomain/block",
            post(block_entry).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route(
            "/api/v2/abuse/admin/entries/:subdomain/data",
            delete(purge_data),
        )
        .route(
            "/api/v2/abuse/admin/entries/:subdomain/requests",
            get(list_entry_requests),
        )
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub subdomain: String,
    pub category: Category,
    pub description: String,
    #[serde(default)]
    pub evidence_url: Option<String>,
    #[serde(default)]
    pub contact: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListEntriesQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct BlockRequest {
    pub admin_token: Option<String>,
    pub category: Option<Category>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntryRequestsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// An entry without its reports, for list responses.
#[derive(Debug, Serialize)]
struct EntrySummary<'a> {
    subdomain: &'a str,
    category: Category,
    source: Source,
    created_at: i64,
    report_count: u32,
}

impl<'a> From<&'a Entry> for EntrySummary<'a> {
    fn from(entry: &'a Entry) -> Self {
        Self {
            subdomain: &entry.subdomain,
            category: entry.category,
            source: entry.source,
            created_at: entry.created_at,
            report_count: entry.report_count,
        }
    }
}

// ============================================================================
// Public Reporting
// ============================================================================

/// POST /api/v2/abuse/reports - Report a subdomain for abuse, which blocks it
pub async fn create_report(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<CreateReportRequest>,
) -> Response {
    if !state.abuse.reporting_enabled() {
        return error(
            StatusCode::NOT_FOUND,
            "reporting_disabled",
            "Abuse reporting is disabled on this instance",
        );
    }

    // Validation comes first, so that a typo does not cost the reporter one of their few
    // attempts.
    let Some(subdomain) = normalize_subdomain(&body.subdomain) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_subdomain",
            "Enter a subdomain of this instance",
        );
    };
    let Some(description) = bounded(&body.description, MAX_DESCRIPTION_CHARS)
        .ok()
        .filter(|description| !description.is_empty())
    else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_description",
            format!(
                "Description is required and must be at most {MAX_DESCRIPTION_CHARS} characters"
            ),
        );
    };
    let evidence_url = match normalize_evidence_url(body.evidence_url.as_deref()) {
        Ok(url) => url,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_evidence_url",
                "Evidence URL must be an http or https URL",
            )
        }
    };
    let Ok(contact) = optional(body.contact.as_deref(), MAX_CONTACT_CHARS) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_contact",
            format!("Contact must be at most {MAX_CONTACT_CHARS} characters"),
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
        description,
        evidence_url,
        contact,
    };
    match state.abuse.report(&subdomain, report).await {
        Ok(applied) => {
            if applied.value == ReportOutcome::Blocked {
                announce_block(&state.tx, &subdomain, body.category);
            }
            let status = match applied.value {
                ReportOutcome::Blocked => StatusCode::CREATED,
                ReportOutcome::AlreadyBlocked => StatusCode::OK,
            };
            (
                status,
                Json(json!({
                    "status": applied.value,
                    "subdomain": subdomain,
                    "persisted": applied.persisted,
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
            "error": "Too many reports from your network",
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

    let suffix = format!(".{}", CONFIG.server_domain);
    let label = host.strip_suffix(&suffix).unwrap_or(host);
    verify_subdomain(
        label,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    )
    .then(|| label.to_string())
}

/// Trims `value` and enforces a character limit. Limits count characters, not bytes.
fn bounded(value: &str, max_chars: usize) -> Result<String, ()> {
    let value = value.trim();
    if value.chars().count() > max_chars {
        return Err(());
    }
    Ok(value.to_string())
}

/// Like `bounded`, but a blank value means the field was left out.
fn optional(value: Option<&str>, max_chars: usize) -> Result<Option<String>, ()> {
    let value = match value {
        None => return Ok(None),
        Some(value) => bounded(value, max_chars)?,
    };
    Ok((!value.is_empty()).then_some(value))
}

/// Evidence links are shown as links in the admin UI, so they must be absolute http(s) URLs.
fn normalize_evidence_url(value: Option<&str>) -> Result<Option<String>, ()> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let uri = Uri::from_str(value).map_err(|_| ())?;
    let is_http = matches!(uri.scheme_str(), Some("http" | "https"));
    if !is_http || uri.host().is_none() || value.chars().count() > MAX_URL_CHARS {
        return Err(());
    }
    Ok(Some(value.to_string()))
}

// ============================================================================
// Admin
// ============================================================================

/// Requires a valid admin token, supplied as `Authorization: Bearer`, the `admin_token`
/// cookie, or `admin_token` in the request body.
fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    cookies: &CookieJar,
    body_token: Option<&str>,
) -> Option<Response> {
    state
        .abuse
        .verify_admin(extract_admin_token(body_token, cookies, headers).as_deref())
        .err()
        .map(|err| match err {
            AdminError::NotConfigured => error(
                StatusCode::FORBIDDEN,
                "admin_not_configured",
                "Managing blocked subdomains requires ADMIN_TOKEN to be configured",
            ),
            AdminError::Invalid => error(
                StatusCode::UNAUTHORIZED,
                "invalid_admin_token",
                "A valid admin token is required",
            ),
        })
}

/// GET /api/v2/abuse/admin/entries - Blocked subdomains, newest first
pub async fn list_entries(
    State(state): State<AppState>,
    Query(query): Query<ListEntriesQuery>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Response {
    if let Some(response) = require_admin(&state, &headers, &cookies, None) {
        return response;
    }

    let limit = query
        .limit
        .unwrap_or(DEFAULT_PAGE_LIMIT)
        .clamp(1, MAX_PAGE_LIMIT);
    let offset = query.offset.unwrap_or(0);
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
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.subdomain.cmp(&b.subdomain))
    });

    let total = entries.len();
    let page: Vec<EntrySummary> = entries
        .iter()
        .skip(offset)
        .take(limit)
        .map(|entry| EntrySummary::from(*entry))
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "entries": page,
            "total": total,
            "limit": limit,
            "offset": offset,
        })),
    )
        .into_response()
}

/// GET /api/v2/abuse/admin/entries/:subdomain - One entry, with its reports
pub async fn get_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Response {
    if let Some(response) = require_admin(&state, &headers, &cookies, None) {
        return response;
    }

    match state.abuse.entry(&subdomain.to_lowercase()) {
        Some(entry) => (StatusCode::OK, Json(entry.as_ref())).into_response(),
        None => not_found(),
    }
}

/// POST /api/v2/abuse/admin/entries/:subdomain/block - Block a subdomain directly
pub async fn block_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
    cookies: CookieJar,
    body: Bytes,
) -> Response {
    // The body is optional, so that `curl -X POST -H "Authorization: Bearer …"` is enough.
    // Anything else must parse: `Option<Json<T>>` would silently treat a malformed body
    // as an absent one.
    let BlockRequest {
        admin_token,
        category,
        note,
    } = if body.is_empty() {
        BlockRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(body) => body,
            Err(e) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "invalid_body",
                    format!("Body must be a JSON object: {e}"),
                )
            }
        }
    };
    if let Some(response) = require_admin(&state, &headers, &cookies, admin_token.as_deref()) {
        return response;
    }

    let Some(subdomain) = normalize_subdomain(&subdomain) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_subdomain",
            "Enter a subdomain of this instance",
        );
    };
    let Ok(note) = optional(note.as_deref(), MAX_NOTE_CHARS) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_note",
            format!("Note must be at most {MAX_NOTE_CHARS} characters"),
        );
    };

    let category = category.unwrap_or(Category::Other);
    match state.abuse.block(&subdomain, category, note).await {
        Ok(applied) => {
            if applied.value {
                announce_block(&state.tx, &subdomain, category);
            }
            let status = if applied.value {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (
                status,
                Json(json!({
                    "status": if applied.value { "blocked" } else { "already_blocked" },
                    "subdomain": subdomain,
                    "persisted": applied.persisted,
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

/// DELETE /api/v2/abuse/admin/entries/:subdomain - Lift the block
pub async fn lift_entry(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Response {
    if let Some(response) = require_admin(&state, &headers, &cookies, None) {
        return response;
    }

    let subdomain = subdomain.to_lowercase();
    match state.abuse.lift(&subdomain).await {
        Applied { value: false, .. } => not_found(),
        Applied { persisted, .. } => (
            StatusCode::OK,
            Json(json!({
                "message": "Block lifted",
                "subdomain": subdomain,
                "persisted": persisted,
            })),
        )
            .into_response(),
    }
}

/// DELETE /api/v2/abuse/admin/entries/:subdomain/data - Delete the subdomain's files,
/// DNS records and captured requests
pub async fn purge_data(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Response {
    if let Some(response) = require_admin(&state, &headers, &cookies, None) {
        return response;
    }

    let Some(subdomain) = normalize_subdomain(&subdomain) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_subdomain",
            "Enter a subdomain of this instance",
        );
    };

    let cache = &state.cache;
    let _ = cache.delete(&format!("files:{subdomain}")).await;
    delete_dns_record_entries(cache, &subdomain).await;
    let _ = cache.delete(&format!("dns:{subdomain}")).await;
    let _ = cache.delete(&format!("requests:{subdomain}")).await;

    // The subdomain may be unblocked by now, with an owner's dashboard still showing them.
    let _ = state.tx.send(CacheMessage {
        cmd: "delete_all".to_string(),
        subdomain: subdomain.clone(),
        data: "{}".to_string(),
    });

    (
        StatusCode::OK,
        Json(json!({
            "message": "Deleted the files, DNS records and requests of the subdomain",
            "subdomain": subdomain,
        })),
    )
        .into_response()
}

/// GET /api/v2/abuse/admin/entries/:subdomain/requests - What the subdomain captured
/// before it was blocked. Read-only.
pub async fn list_entry_requests(
    State(state): State<AppState>,
    Path(subdomain): Path<String>,
    Query(query): Query<EntryRequestsQuery>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Response {
    if let Some(response) = require_admin(&state, &headers, &cookies, None) {
        return response;
    }

    let subdomain = subdomain.to_lowercase();
    if state.abuse.entry(&subdomain).is_none() {
        return not_found();
    }

    let limit = query
        .limit
        .unwrap_or(DEFAULT_PAGE_LIMIT)
        .clamp(1, MAX_PAGE_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let (total, requests) = paginated_requests(&state.cache, &subdomain, limit, offset).await;

    (
        StatusCode::OK,
        Json(json!({
            "requests": requests,
            "pagination": {
                "total": total,
                "limit": limit,
                "offset": offset,
                "has_more": offset + limit < total,
            }
        })),
    )
        .into_response()
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

fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "not_found",
        "Subdomain is not blocked",
    )
}

/// Sent to the owner of a subdomain on any endpoint that accepts its token.
pub(crate) fn blocked_rejection(entry: &Entry) -> Response {
    let mut body = json!({
        "error": "This subdomain has been blocked",
        "code": "subdomain_blacklisted",
        "category": entry.category,
    });
    if let Some(contact) = &CONFIG.abuse_contact {
        body["contact"] = json!(contact);
    }
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

/// Sent for any traffic to a blocked subdomain. Plain text on purpose: nothing of the
/// subdomain's own content is served, and the response is not cached.
pub(crate) fn blocked_traffic_response() -> Response {
    let mut body = String::from("410 Gone: this subdomain has been disabled for abuse.");
    if let Some(contact) = &CONFIG.abuse_contact {
        body.push_str(&format!(
            "\nIf you believe this is a mistake, contact {contact}."
        ));
    }

    Response::builder()
        .status(StatusCode::GONE)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        // The request body was never read, so the connection cannot be reused safely.
        .header(header::CONNECTION, "close")
        .body(Body::from(body))
        .unwrap()
}

/// Tells live dashboards and other listeners that `subdomain` is now blocked, and why.
fn announce_block(tx: &broadcast::Sender<CacheMessage>, subdomain: &str, category: Category) {
    let _ = tx.send(CacheMessage {
        cmd: "blacklisted".to_string(),
        subdomain: subdomain.to_string(),
        data: json!({ "category": category }).to_string(),
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
        let foreign = format!("{}.example.org", CONFIG.server_domain);
        assert_eq!(normalize_subdomain(&format!("{label}.example.org")), None);
        assert_eq!(normalize_subdomain(&format!("{label}.{foreign}")), None);
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
    fn evidence_url_must_be_absolute_http_s() {
        assert_eq!(normalize_evidence_url(None), Ok(None));
        assert_eq!(normalize_evidence_url(Some("  ")), Ok(None));
        assert_eq!(
            normalize_evidence_url(Some(" https://threatfox.abuse.ch/ioc/1982219/ ")),
            Ok(Some("https://threatfox.abuse.ch/ioc/1982219/".to_string()))
        );
        assert_eq!(
            normalize_evidence_url(Some("http://example.org")),
            Ok(Some("http://example.org".to_string()))
        );

        assert_eq!(normalize_evidence_url(Some("example.org/report")), Err(()));
        assert_eq!(normalize_evidence_url(Some("javascript:alert(1)")), Err(()));
        assert_eq!(normalize_evidence_url(Some("data:text/html,x")), Err(()));
        assert_eq!(
            normalize_evidence_url(Some(&format!("https://{}", "a".repeat(MAX_URL_CHARS)))),
            Err(())
        );
    }

    #[test]
    fn bounded_trims_and_enforces_limits() {
        assert_eq!(bounded("  spaced out  ", 20), Ok("spaced out".to_string()));
        assert_eq!(bounded("abc", 3), Ok("abc".to_string()));
        assert_eq!(bounded("abcd", 3), Err(()));
        // Limits count characters, not bytes
        assert_eq!(bounded("ééééé", 5), Ok("ééééé".to_string()));
    }

    #[test]
    fn optional_treats_blank_as_absent() {
        assert_eq!(optional(None, 20), Ok(None));
        assert_eq!(optional(Some(""), 20), Ok(None));
        assert_eq!(optional(Some("   "), 20), Ok(None));
        assert_eq!(
            optional(Some(" abuse@example.org "), 20),
            Ok(Some("abuse@example.org".to_string()))
        );
        assert_eq!(optional(Some("abcd"), 3), Err(()));
    }
}
