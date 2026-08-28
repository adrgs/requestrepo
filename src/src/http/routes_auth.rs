use axum::extract::Path;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, info};

use crate::auth::{github, oidc, session};
use crate::cache::Cache;
use crate::utils::config::CONFIG;

#[derive(Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub icon: String,
    pub login_url: String,
    pub provider_type: String,
}

#[derive(Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub authenticated: bool,
    pub user: Option<session::UserProfile>,
}

#[derive(Serialize)]
pub struct TokenCheckResponse {
    pub valid: bool,
    pub needs_refresh: bool,
}

fn get_session_token(headers: &HeaderMap, cookies: &axum_extra::extract::CookieJar) -> Option<String> {
    // Try cookie first
    if let Some(cookie) = cookies.get("session_token") {
        return Some(cookie.value().to_string());
    }
    // Try Authorization header
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub async fn get_providers() -> impl IntoResponse {
    let mut providers = Vec::new();

    // GitHub
    if CONFIG.github_enabled && !CONFIG.github_client_id.is_empty() {
        let state = session::generate_session_token();
        providers.push(ProviderInfo {
            name: "github".to_string(),
            display_name: "GitHub".to_string(),
            icon: "github.svg".to_string(),
            login_url: format!("/auth/github?state={state}"),
            provider_type: "github".to_string(),
        });
    }

    // OIDC providers
    for provider in &CONFIG.oidc_providers {
        let state = session::generate_session_token();
        providers.push(ProviderInfo {
            name: provider.name.clone(),
            display_name: provider.display_name.clone(),
            icon: format!("{}.svg", provider.name),
            login_url: format!("/auth/oidc/{}?state={state}", provider.name),
            provider_type: "oidc".to_string(),
        });
    }

    Json(ProvidersResponse { providers })
}

pub async fn github_login() -> impl IntoResponse {
    if CONFIG.github_client_id.is_empty() {
        return Redirect::temporary("/login?error=github_not_configured").into_response();
    }

    let state = session::generate_session_token();
    let url = github::build_github_authorize_url(&state);

    let mut response = Redirect::temporary(&url).into_response();
    // Store state in cookie for CSRF verification
    let cookie = format!("oauth_state={state}; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=600");
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().unwrap());
    response
}

pub async fn github_callback(
    axum::extract::Query(params): axum::extract::Query<github::GitHubCallbackParams>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> Response {
    github::handle_github_callback(params, cache).await
}

pub async fn oidc_login(
    Path(provider_name): Path<String>,
) -> Response {
    let provider = match oidc::find_provider(&provider_name) {
        Some(p) => p,
        None => {
            return Redirect::temporary("/login?error=unknown_provider").into_response();
        }
    };

    let discovery = match oidc::discover(provider).await {
        Ok(d) => d,
        Err(e) => {
            error!("OIDC discovery failed for {provider_name}: {e}");
            return Redirect::temporary("/login?error=discovery_failed").into_response();
        }
    };

    let state = session::generate_session_token();
    let redirect_uri = format!(
        "https://{}/auth/oidc/{}/callback",
        CONFIG.server_domain, provider_name
    );

    let url = oidc::build_oidc_authorize_url(provider, &discovery, &state, &redirect_uri);

    let mut response = Redirect::temporary(&url).into_response();
    let cookie = format!(
        "oauth_state_{provider_name}={state}; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=600"
    );
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().unwrap());
    response
}

pub async fn oidc_callback(
    Path(provider_name): Path<String>,
    axum::extract::Query(params): axum::extract::Query<oidc::OidcCallbackParams>,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> Response {
    oidc::handle_oidc_callback(&provider_name, params, cache).await
}

pub async fn get_user(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    let token = match get_session_token(&headers, &cookies) {
        Some(t) => t,
        None => {
            return Json(UserResponse {
                authenticated: false,
                user: None,
            });
        }
    };

    match session::get_session(&cache, &token).await {
        Some(sess) => {
            let profile = session::UserProfile {
                username: sess.username,
                avatar_url: sess.avatar_url,
                name: sess.name,
                created_at: sess.created_at,
                is_admin: sess.is_admin,
            };
            Json(UserResponse {
                authenticated: true,
                user: Some(profile),
            })
        }
        None => Json(UserResponse {
            authenticated: false,
            user: None,
        }),
    }
}

pub async fn check_token(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    let token = match get_session_token(&headers, &cookies) {
        Some(t) => t,
        None => {
            return Json(TokenCheckResponse {
                valid: false,
                needs_refresh: false,
            });
        }
    };

    match session::get_session(&cache, &token).await {
        Some(_) => Json(TokenCheckResponse {
            valid: true,
            needs_refresh: false,
        }),
        None => Json(TokenCheckResponse {
            valid: false,
            needs_refresh: false,
        }),
    }
}

pub async fn refresh_token(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> impl IntoResponse {
    // Session-based auth doesn't need refresh - last_accessed is updated on each get_session call
    let token = match get_session_token(&headers, &cookies) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "No session"})),
            );
        }
    };

    match session::get_session(&cache, &token).await {
        Some(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"msg": "Token is still valid"})),
        ),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid session"})),
        ),
    }
}

pub async fn logout(
    headers: HeaderMap,
    cookies: axum_extra::extract::CookieJar,
    axum::extract::Extension(cache): axum::extract::Extension<Arc<Cache>>,
) -> Response {
    if let Some(token) = get_session_token(&headers, &cookies) {
        let _ = session::delete_session(&cache, &token).await;
    }

    let mut response = Redirect::temporary("/login").into_response();
    // Clear the session cookie
    response.headers_mut().insert(
        header::SET_COOKIE,
        "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    response
}
