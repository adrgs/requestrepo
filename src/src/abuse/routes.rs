//! HTTP endpoints for reporting abuse.
//!
//! Neither route is CORS-enabled, and reports must be `application/json`, which a cross-site
//! form or beacon cannot send without a CORS preflight: no website can make its visitors'
//! browsers file reports.

use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tracing::warn;

use super::{REPORT_RATE_LIMIT, REPORT_RATE_WINDOW};
use crate::http::routes_v2::json_body;
use crate::http::AppState;
use crate::models::CacheMessage;
use crate::utils::auth::extract_admin_token;
use crate::utils::config::CONFIG;
use crate::utils::verify_subdomain;

/// Largest accepted report body. It holds one short field.
const MAX_BODY_BYTES: usize = 1024;
const MAX_SUBDOMAIN_INPUT: usize = 512;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v2/abuse", get(policy)).route(
        "/api/v2/abuse/reports",
        post(create_report).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReportRequest {
    pub subdomain: String,
    /// The ADMIN_TOKEN of a private instance, for clients that cannot send a header.
    #[serde(default)]
    pub admin_token: Option<String>,
}

/// GET /api/v2/abuse - How reporting works on this instance
pub async fn policy(State(state): State<AppState>) -> Response {
    let settings = state.abuse.settings();
    Json(json!({
        "reporting_enabled": settings.reporting_enabled,
        "requires_admin_token": settings.admin_token.is_some(),
        "effect": "A report suspends the subdomain immediately.",
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

/// POST /api/v2/abuse/reports - Report a subdomain, which suspends it at once
pub async fn create_report(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: CookieJar,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let settings = state.abuse.settings();
    if !settings.reporting_enabled {
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

    // A private instance takes reports only from its own users, as it does sessions.
    let token = extract_admin_token(body.admin_token.as_deref(), &cookies, &headers);
    if !settings.may_report(token.as_deref()) {
        return error(
            StatusCode::FORBIDDEN,
            "admin_required",
            "This instance is private: send its admin token to report abuse",
        );
    }

    // Validation comes first, so that a typo does not cost the reporter an attempt.
    let Some(subdomain) = normalize_subdomain(&body.subdomain) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_subdomain",
            format!(
                "Give a subdomain of {}: a label, a hostname or a URL",
                CONFIG.server_domain
            ),
        );
    };

    if let Err(retry_after) = state.abuse.check_report_rate(addr.ip()) {
        return rate_limited(retry_after);
    }

    let newly = state.abuse.suspend(&subdomain).await;
    if newly {
        warn!("Subdomain {subdomain} suspended by an abuse report");
        let _ = state.tx.send(CacheMessage {
            cmd: "suspended".to_string(),
            subdomain: subdomain.clone(),
            data: "{}".to_string(),
        });
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
        })),
    )
        .into_response()
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

/// Parses what a reporter typed for the subdomain: the label (`abc12345`), a hostname under
/// it (`x.abc12345.example.com`, with or without a port) or a URL. Only subdomains of this
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

    // A name under the subdomain belongs to it too.
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

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "error": message.into(), "code": code })),
    )
        .into_response()
}

/// Sent to the owner of a suspended subdomain on any endpoint that accepts its token.
pub(crate) fn suspended_rejection() -> Response {
    let mut body = json!({
        "error": "This subdomain was suspended after an abuse report. Create a new session.",
        "code": "subdomain_suspended",
    });
    if let Some(contact) = &CONFIG.abuse_contact {
        body["contact"] = json!(contact);
    }
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

/// Sent for any traffic to a suspended subdomain: a small page for browsers, plain text for
/// everything else. Nothing is read, logged or served, and nothing is cached.
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
        // The request body was never read, so the connection cannot be reused safely.
        .header(header::CONNECTION, "close")
        .body(Body::from(body))
        .unwrap()
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
        assert_eq!(
            normalize_subdomain(&format!("data.x.{label}.{domain}")),
            expected
        );
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
        assert_eq!(normalize_subdomain(&format!("{label}.example.org")), None);
        assert_eq!(normalize_subdomain(&CONFIG.server_domain.clone()), None);
        assert_eq!(
            normalize_subdomain(&format!("x.{}", CONFIG.server_domain)),
            None
        );
        assert_eq!(normalize_subdomain(&format!("{}x", label)), None);
        assert_eq!(
            normalize_subdomain(&format!("{label}.{}/path", CONFIG.server_domain)),
            None
        );
        assert_eq!(normalize_subdomain("http://"), None);
    }

    #[test]
    fn the_suspended_page_escapes_the_contact() {
        let response = suspended_traffic_response(Some("text/html"));
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(html_escape("<a&\">"), "&lt;a&amp;&quot;&gt;");
    }
}
