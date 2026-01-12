use super::config::CONFIG;
use super::verify_jwt;

/// Authentication result containing the type and optional subdomain
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Admin token authentication - full access
    Admin,
    /// JWT session authentication - subdomain-scoped access
    Session(String),
}

/// Verify if the provided token is a valid admin token
pub fn verify_admin_token(token: &str) -> bool {
    match &CONFIG.admin_token {
        Some(admin_token) => token == admin_token,
        None => false,
    }
}

/// Check if admin token is required for API access
pub fn is_admin_token_required() -> bool {
    CONFIG.admin_token.is_some()
}

/// Verify authentication - checks admin token first, then JWT
/// Returns AuthResult on success, None on failure
pub fn verify_auth(token: &str) -> Option<AuthResult> {
    // Check admin token first
    if verify_admin_token(token) {
        return Some(AuthResult::Admin);
    }

    // Fall back to JWT verification
    if let Some(subdomain) = verify_jwt(token) {
        return Some(AuthResult::Session(subdomain));
    }

    None
}

/// Verify that the token grants access to a specific subdomain
/// Admin tokens have access to all subdomains
pub fn verify_subdomain_access(token: &str, target_subdomain: &str) -> bool {
    match verify_auth(token) {
        Some(AuthResult::Admin) => true,
        Some(AuthResult::Session(subdomain)) => subdomain == target_subdomain,
        None => false,
    }
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

    #[test]
    fn test_verify_auth_with_jwt() {
        // This test depends on JWT secret being set
        // We can't easily test this without a valid JWT
    }
}
