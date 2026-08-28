use crate::auth::session;
use crate::cache::Cache;
use crate::utils::config::CONFIG;
use axum::http::header;
use axum::response::{IntoResponse, Redirect, Response};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct GitHubCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GitHubTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
}

pub fn build_github_authorize_url(state: &str) -> String {
    let client_id = &CONFIG.github_client_id;
    let callback_url = if CONFIG.github_callback_url.is_empty() {
        format!("https://{}/auth/github/callback", CONFIG.server_domain)
    } else {
        CONFIG.github_callback_url.clone()
    };

    format!(
        "https://github.com/login/oauth/authorize?client_id={client_id}&scope=read%20user%3Aemail&state={state}&redirect_uri={callback_url}"
    )
}

pub async fn exchange_code(code: &str) -> Result<GitHubTokenResponse, String> {
    let client_id = &CONFIG.github_client_id;
    let client_secret = &CONFIG.github_client_secret;

    if client_id.is_empty() || client_secret.is_empty() {
        return Err("GitHub OAuth not configured".to_string());
    }

    let client = Client::new();
    let mut params = HashMap::new();
    params.insert("client_id", client_id.as_str());
    params.insert("client_secret", client_secret.as_str());
    params.insert("code", code);

    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("accept", "application/json")
        .json(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange code: {e}"))?;

    let body: GitHubTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

    Ok(body)
}

pub async fn get_github_user(access_token: &str) -> Result<GitHubUser, String> {
    let client = Client::new();
    let resp = client
        .get("https://api.github.com/user")
        .header("authorization", format!("Bearer {access_token}"))
        .header("accept", "application/json")
        .header("user-agent", "requestrepo")
        .send()
        .await
        .map_err(|e| format!("Failed to get user info: {e}"))?;

    let user: GitHubUser = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse user info: {e}"))?;

    Ok(user)
}

pub async fn handle_github_callback(
    params: GitHubCallbackParams,
    cache: Arc<Cache>,
) -> Response {
    let code = match params.code {
        Some(c) => c,
        None => {
            error!("GitHub callback missing code parameter");
            return Redirect::temporary("/login?error=no_code").into_response();
        }
    };

    // Exchange code for token
    let token_resp = match exchange_code(&code).await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to exchange GitHub code: {e}");
            return Redirect::temporary("/login?error=token_exchange_failed").into_response();
        }
    };

    // Get user info
    let user = match get_github_user(&token_resp.access_token).await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to get GitHub user: {e}");
            return Redirect::temporary("/login?error=user_info_failed").into_response();
        }
    };

    let username = user.login.to_lowercase();

    // Check if user is allowed
    if !session::is_user_allowed(&username, &CONFIG.github_allowed_usernames) {
        info!("User {username} not in allowed list, denying access");
        return Redirect::temporary("/login?error=unauthorized").into_response();
    }

    let is_admin = session::is_user_admin(&username, &CONFIG.allowed_admins);

    let name = user.name.unwrap_or_else(|| username.clone());
    let avatar_url = user.avatar_url.unwrap_or_default();

    // Register/update user
    let profile = session::UserProfile {
        username: username.clone(),
        avatar_url: avatar_url.clone(),
        name: name.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        is_admin,
    };
    let _ = session::register_user(&cache, &profile).await;

    // Create session
    let session_token = match session::create_session(&cache, &username, &avatar_url, &name, is_admin).await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to create session: {e}");
            return Redirect::temporary("/login?error=session_failed").into_response();
        }
    };

    info!("GitHub login successful for user {username}");

    // Set cookie and redirect
    let session_ttl: u64 = 90 * 24 * 60 * 60;
    let cookie = format!(
        "session_token={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={session_ttl}"
    );

    let mut response = Redirect::temporary("/").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie.parse().unwrap(),
    );
    response
}
