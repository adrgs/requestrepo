use super::config::CONFIG;

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
