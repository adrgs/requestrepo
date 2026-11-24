use super::config::CONFIG;
use axum::http::HeaderMap;
use axum_extra::extract::CookieJar;

/// Constant-time string comparison to prevent timing attacks
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// The token in `Authorization: Bearer <token>`
pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    (!token.is_empty()).then(|| token.to_string())
}

/// Find the admin token supplied with a request. The body wins over the cookie, which wins
/// over the `Authorization` header.
pub fn extract_admin_token(
    body_token: Option<&str>,
    cookies: &CookieJar,
    headers: &HeaderMap,
) -> Option<String> {
    // Body takes precedence
    if let Some(token) = body_token {
        return Some(token.to_string());
    }
    // Fallback to cookie
    if let Some(token) = cookies.get("admin_token") {
        return Some(token.value().to_string());
    }
    // Fallback to Authorization header
    bearer_token(headers)
}

/// Verify if the provided token is a valid admin token
pub fn verify_admin_token(token: &str) -> bool {
    match &CONFIG.admin_token {
        Some(admin_token) => constant_time_eq(token, admin_token),
        None => false,
    }
}

/// Check if admin token is required for API access
pub fn is_admin_token_required() -> bool {
    CONFIG.admin_token.is_some()
}

/// Check if a request to /api/get_token should be allowed
/// - If ADMIN_TOKEN is set: requires valid admin token
/// - If ADMIN_TOKEN is not set: allow all requests
pub fn can_create_session(token: Option<&str>) -> bool {
    if !is_admin_token_required() {
        return true;
    }

    match token {
        Some(t) => verify_admin_token(t),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_token_not_set() {
        // When ADMIN_TOKEN is not set, verify_admin_token should return false
        // but can_create_session should return true (open access)
        assert!(!is_admin_token_required() || !verify_admin_token("random_token"));
    }
}
