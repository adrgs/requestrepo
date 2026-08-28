use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

use crate::auth::session;
use crate::cache::Cache;
use crate::utils::{config::CONFIG, generate_jwt, verify_subdomain, write_basic_file};

#[derive(Serialize)]
pub struct AdminUserResponse {
    pub username: String,
    pub avatar_url: String,
    pub name: String,
    pub created_at: String,
    pub is_admin: bool,
}

#[derive(Serialize)]
pub struct AdminSubdomainResponse {
    pub subdomain: String,
}

#[derive(Serialize)]
pub struct AdminConfigResponse {
    pub subdomain_length: usize,
    pub subdomain_alphabet: String,
}

fn get_session_token(headers: &HeaderMap, cookies: &axum_extra::extract::CookieJar) -> Option<String> {
    if let Some(cookie) = cookies.get("session_token") {
        return Some(cookie.value().to_string());
    }
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    None
}

async fn require_admin(
    headers: &HeaderMap,
    cookies: &axum_extra::extract::CookieJar,
    cache: &Arc<Cache>,
) -> Result<session::UserSession, (StatusCode, Json<serde_json::Value>)> {
    let token = get_session_token(headers, cookies).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Not authenticated"})),
        )
    })?;

    let sess = session::get_session(cache, &token).await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid session"})),
        )
    })?;

    if !sess.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Admin access required"})),
        ));
    }

    Ok(sess)
}

pub async fn get_users(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    let users = session::get_all_users(&cache).await;
    let response: Vec<AdminUserResponse> = users
        .into_iter()
        .map(|u| AdminUserResponse {
            username: u.username,
            avatar_url: u.avatar_url,
            name: u.name,
            created_at: u.created_at,
            is_admin: u.is_admin,
        })
        .collect();

    Json(response).into_response()
}

pub async fn get_subdomains(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    // Find all subdomains by looking for files:{subdomain} keys
    match cache.keys("files:*").await {
        Ok(keys) => {
            let subdomains: Vec<AdminSubdomainResponse> = keys
                .iter()
                .filter_map(|k| k.strip_prefix("files:"))
                .map(|s| AdminSubdomainResponse {
                    subdomain: s.to_string(),
                })
                .collect();
            Json(subdomains).into_response()
        }
        Err(_) => Json(Vec::<AdminSubdomainResponse>::new()).into_response(),
    }
}

pub async fn get_config(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    Json(AdminConfigResponse {
        subdomain_length: CONFIG.subdomain_length,
        subdomain_alphabet: CONFIG.subdomain_alphabet.clone(),
    })
    .into_response()
}

pub async fn get_logs(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    Path(subdomain): Path<String>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    if !verify_subdomain(
        &subdomain,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid subdomain"})),
        )
            .into_response();
    }

    let key = format!("requests:{subdomain}");
    match cache.lrange(&key, 0, -1).await {
        Ok(items) => {
            let logs: Vec<serde_json::Value> = items
                .iter()
                .filter(|s| s != &&"{}".to_string())
                .filter_map(|s| serde_json::from_str(s).ok())
                .collect();
            Json(logs).into_response()
        }
        Err(_) => Json(Vec::<serde_json::Value>::new()).into_response(),
    }
}

pub async fn delete_subdomain(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    Path(subdomain): Path<String>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    if !verify_subdomain(
        &subdomain,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid subdomain"})),
        )
            .into_response();
    }

    // Delete files
    let _ = cache.delete(&format!("files:{subdomain}")).await;

    // Delete DNS records
    if let Ok(Some(dns_json)) = cache.get(&format!("dns:{subdomain}")).await {
        if let Ok(records) = serde_json::from_str::<Vec<serde_json::Value>>(&dns_json) {
            for record in &records {
                if let (Some(t), Some(d)) = (
                    record.get("type").and_then(|v| v.as_str()),
                    record.get("domain").and_then(|v| v.as_str()),
                ) {
                    let _ = cache.delete(&format!("dns:{t}:{d}")).await;
                }
            }
        }
    }
    let _ = cache.delete(&format!("dns:{subdomain}")).await;

    // Delete requests
    let _ = cache.delete(&format!("requests:{subdomain}")).await;

    // Delete notification settings
    let _ = cache.delete(&format!("notifications:{subdomain}")).await;

    info!("Admin deleted subdomain {subdomain}");

    Json(serde_json::json!({"msg": format!("Subdomain {subdomain} deleted successfully")})).into_response()
}

pub async fn generate_token(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    Path(subdomain): Path<String>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    if !verify_subdomain(
        &subdomain,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("Invalid subdomain. Must be exactly {} characters.", CONFIG.subdomain_length)})),
        )
            .into_response();
    }

    // Create subdomain if it doesn't exist
    let files_key = format!("files:{subdomain}");
    if cache.get(&files_key).await.ok().flatten().is_none() {
        let _ = write_basic_file(&subdomain, &cache).await;
    }

    // Generate JWT
    let token = match generate_jwt(&subdomain) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to generate token: {e}")})),
            )
                .into_response();
        }
    };

    info!("Admin generated token for subdomain {subdomain}");

    Json(serde_json::json!({"token": token, "subdomain": subdomain})).into_response()
}

pub async fn delete_all_subdomains(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    let subdomains = match cache.keys("files:*").await {
        Ok(keys) => keys
            .iter()
            .filter_map(|k| k.strip_prefix("files:").map(|s| s.to_string()))
            .collect::<Vec<_>>(),
        Err(_) => return Json(serde_json::json!({"msg": "No subdomains found"})).into_response(),
    };

    let count = subdomains.len();
    for subdomain in &subdomains {
        let _ = cache.delete(&format!("files:{subdomain}")).await;
        if let Ok(Some(dns_json)) = cache.get(&format!("dns:{subdomain}")).await {
            if let Ok(records) = serde_json::from_str::<Vec<serde_json::Value>>(&dns_json) {
                for record in &records {
                    if let (Some(t), Some(d)) = (
                        record.get("type").and_then(|v| v.as_str()),
                        record.get("domain").and_then(|v| v.as_str()),
                    ) {
                        let _ = cache.delete(&format!("dns:{t}:{d}")).await;
                    }
                }
            }
        }
        let _ = cache.delete(&format!("dns:{subdomain}")).await;
        let _ = cache.delete(&format!("requests:{subdomain}")).await;
        let _ = cache.delete(&format!("notifications:{subdomain}")).await;
    }

    info!("Admin deleted all {count} subdomains");
    Json(serde_json::json!({"msg": format!("Deleted {count} subdomains")})).into_response()
}

pub async fn delete_all_logs(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    Path(subdomain): Path<String>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    if !verify_subdomain(
        &subdomain,
        CONFIG.subdomain_length,
        &CONFIG.subdomain_alphabet_set,
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid subdomain"})),
        )
            .into_response();
    }

    let _ = cache.delete(&format!("requests:{subdomain}")).await;
    info!("Admin deleted all logs for subdomain {subdomain}");
    Json(serde_json::json!({"msg": format!("All logs deleted for {subdomain}")})).into_response()
}

pub async fn delete_log(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    Path((subdomain, log_id)): Path<(String, String)>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&headers, &cookies, &cache).await {
        return e.into_response();
    }

    let key = format!("requests:{subdomain}");
    if let Ok(items) = cache.lrange(&key, 0, -1).await {
        for (idx, item) in items.iter().enumerate() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(item) {
                if val.get("_id").and_then(|v| v.as_str()) == Some(&log_id) {
                    let _ = cache.lset(&key, idx as isize, "{}").await;
                    let _ = cache.delete(&format!("request:{subdomain}:{log_id}")).await;
                    return Json(serde_json::json!({"msg": "Log deleted"})).into_response();
                }
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "Log not found"})),
    )
        .into_response()
}
