
#[cfg(test)]
mod tests {
    use crate::utils::{
        generate_jwt, get_random_subdomain, get_subdomain_from_hostname,
        get_subdomain_from_path, verify_jwt, verify_subdomain,
    };
    use crate::utils::config::CONFIG;
    use std::collections::HashSet;

    #[test]
    fn test_verify_subdomain() {
        let alphabet_set: HashSet<char> = CONFIG.subdomain_alphabet.chars().collect();
        
        assert!(verify_subdomain("abcdefgh", 8, &alphabet_set));
        
        assert!(!verify_subdomain("abcdefg", 8, &alphabet_set));
        assert!(!verify_subdomain("abcdefghi", 8, &alphabet_set));
        
        assert!(!verify_subdomain("abcdefg!", 8, &alphabet_set));
    }

    #[test]
    fn test_get_random_subdomain() {
        let subdomain = get_random_subdomain();
        
        assert_eq!(subdomain.len(), CONFIG.subdomain_length);
        
        for c in subdomain.chars() {
            assert!(CONFIG.subdomain_alphabet_set.contains(&c));
        }
    }

    #[test]
    fn test_get_subdomain_from_hostname() {
        let subdomain = get_subdomain_from_hostname(&format!("abcdefgh.{}", CONFIG.server_domain));
        assert_eq!(subdomain, Some("abcdefgh".to_string()));
        
        let subdomain = get_subdomain_from_hostname("invalid.example.com");
        assert_eq!(subdomain, None);
        
        let subdomain = get_subdomain_from_hostname("");
        assert_eq!(subdomain, None);
    }

    #[test]
    fn test_get_subdomain_from_path() {
        let subdomain = get_subdomain_from_path("/r/abcdefgh");
        assert_eq!(subdomain, Some("abcdefgh".to_string()));
        
        let subdomain = get_subdomain_from_path("/invalid");
        assert_eq!(subdomain, None);
        
        let subdomain = get_subdomain_from_path("");
        assert_eq!(subdomain, None);
    }

    #[test]
    fn test_jwt() {
        let subdomain = "abcdefgh";
        
        let token = generate_jwt(subdomain).unwrap();
        
        let verified_subdomain = verify_jwt(&token);
        assert_eq!(verified_subdomain, Some(subdomain.to_string()));
        
        let verified_subdomain = verify_jwt("invalid.token");
        assert_eq!(verified_subdomain, None);
    }
}
