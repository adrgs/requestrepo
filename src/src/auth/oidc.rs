use crate::auth::session;
use crate::cache::Cache;
use crate::utils::config::{CONFIG, OidcProviderConfig};
use axum::response::{IntoResponse, Redirect, Response};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct OidcDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OidcTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OidcUserInfo {
    pub sub: Option<String>,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
}

pub fn find_provider(name: &str) -> Option<&'static OidcProviderConfig> {
    CONFIG.oidc_providers.iter().find(|p| p.name == name)
}

pub async fn discover(provider: &OidcProviderConfig) -> Result<OidcDiscovery, String> {
    // Well-known URL is the base + /.well-known/openid-configuration
    let base_url = provider
        .discover_url
        .trim_end_matches("/.well-known/openid-configuration")
        .trim_end_matches('/');
    let well_known_url = format!("{base_url}/.well-known/openid-configuration");

    let client = Client::new();
    let resp = client
        .get(&well_known_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch OIDC discovery: {e}"))?;

    let discovery: OidcDiscovery = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OIDC discovery: {e}"))?;

    Ok(discovery)
}

pub fn build_oidc_authorize_url(
    provider: &OidcProviderConfig,
    discovery: &OidcDiscovery,
    state: &str,
    redirect_uri: &str,
) -> String {
    format!(
        "{}?client_id={}&response_type=code&scope=openid%20email%20profile&state={}&redirect_uri={}",
        discovery.authorization_endpoint,
        provider.client_id,
        state,
        urlencoding::encode(redirect_uri),
    )
}

pub async fn exchange_code(
    provider: &OidcProviderConfig,
    discovery: &OidcDiscovery,
    code: &str,
    redirect_uri: &str,
) -> Result<OidcTokenResponse, String> {
    let client = Client::new();
    let mut params = HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("client_id", provider.client_id.as_str());
    params.insert("client_secret", provider.client_secret.as_str());
    params.insert("code", code);
    params.insert("redirect_uri", redirect_uri);

    let resp = client
        .post(&discovery.token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange OIDC code: {e}"))?;

    let body: OidcTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OIDC token response: {e}"))?;

    Ok(body)
}

pub async fn get_userinfo(
    _provider: &OidcProviderConfig,
    discovery: &OidcDiscovery,
    access_token: &str,
) -> Result<OidcUserInfo, String> {
    let client = Client::new();
    let resp = client
        .get(&discovery.userinfo_endpoint)
        .header("authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| format!("Failed to get OIDC userinfo: {e}"))?;

    let user: OidcUserInfo = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OIDC userinfo: {e}"))?;

    Ok(user)
}

pub async fn handle_oidc_callback(
    provider_name: &str,
    params: OidcCallbackParams,
    cache: Arc<Cache>,
) -> Response {
    let provider = match find_provider(provider_name) {
        Some(p) => p,
        None => {
            error!("Unknown OIDC provider: {provider_name}");
            return Redirect::temporary("/login?error=unknown_provider").into_response();
        }
    };

    let code = match params.code {
        Some(c) => c,
        None => {
            error!("OIDC callback missing code");
            return Redirect::temporary("/login?error=no_code").into_response();
        }
    };

    let discovery = match discover(provider).await {
        Ok(d) => d,
        Err(e) => {
            error!("OIDC discovery failed: {e}");
            return Redirect::temporary("/login?error=discovery_failed").into_response();
        }
    };

    let redirect_uri = format!(
        "https://{}/auth/oidc/{}/callback",
        CONFIG.server_domain, provider_name
    );

    // Exchange code for token
    let token_resp = match exchange_code(provider, &discovery, &code, &redirect_uri).await {
        Ok(t) => t,
        Err(e) => {
            error!("OIDC token exchange failed: {e}");
            return Redirect::temporary("/login?error=token_exchange_failed").into_response();
        }
    };

    // Get user info
    let user = match get_userinfo(provider, &discovery, &token_resp.access_token).await {
        Ok(u) => u,
        Err(e) => {
            error!("OIDC userinfo failed: {e}");
            return Redirect::temporary("/login?error=userinfo_failed").into_response();
        }
    };

    let username = user
        .preferred_username
        .or(user.email)
        .unwrap_or_else(|| user.sub.unwrap_or_default())
        .to_lowercase();

    if username.is_empty() {
        return Redirect::temporary("/login?error=no_username").into_response();
    }

    // Check allowed users
    if !session::is_user_allowed(&username, &provider.allowed_users) {
        info!("OIDC user {username} not in allowed list for provider {provider_name}");
        return Redirect::temporary("/login?error=unauthorized").into_response();
    }

    let is_admin = session::is_user_admin(&username, &provider.allowed_admins)
        || session::is_user_admin(&username, &CONFIG.allowed_admins);

    let name = user.name.unwrap_or_else(|| username.clone());
    let avatar_url = user.picture.unwrap_or_default();

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

    info!("OIDC login successful for user {username} via {provider_name}");

    let session_ttl: u64 = 90 * 24 * 60 * 60;
    let cookie = format!(
        "session_token={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={session_ttl}"
    );

    let mut response = Redirect::temporary("/").into_response();
    response
        .headers_mut()
        .insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());
    response
}
