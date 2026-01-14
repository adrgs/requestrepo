pub mod auth;
pub mod config;

use crate::models::{Claims, ShareClaims};
use config::CONFIG;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

pub fn verify_subdomain(
    subdomain: &str,
    length: usize,
    alphabet_set: &std::collections::HashSet<char>,
) -> bool {
    subdomain.len() == length && subdomain.chars().all(|c| alphabet_set.contains(&c))
}

pub fn verify_jwt(token: &str) -> Option<String> {
    let validation = Validation::default();
    let key = DecodingKey::from_secret(CONFIG.jwt_secret.as_bytes());

    match decode::<Claims>(token, &key, &validation) {
        Ok(token_data) => {
            let subdomain = token_data.claims.subdomain;
            if verify_subdomain(
                &subdomain,
                CONFIG.subdomain_length,
                &CONFIG.subdomain_alphabet_set,
            ) {
                Some(subdomain)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

pub fn get_random_subdomain() -> String {
    let mut rng = rand::thread_rng();
    let alphabet = CONFIG.subdomain_alphabet.as_bytes();

    (0..CONFIG.subdomain_length)
        .map(|_| {
            let idx = rng.gen_range(0..alphabet.len());
            alphabet[idx] as char
        })
        .collect()
}

pub fn get_subdomain_from_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }

    let path = path.to_lowercase();
    let path = path.trim_start_matches('/');

    if !path.starts_with("r/") {
        return None;
    }

    let path = path[2..].trim_start_matches('/');
    let subdomain = path
        .chars()
        .take(CONFIG.subdomain_length)
        .collect::<String>();

    if subdomain.len() != CONFIG.subdomain_length
        || !subdomain
            .chars()
            .all(|c| CONFIG.subdomain_alphabet_set.contains(&c))
    {
        return None;
    }

    Some(subdomain)
}

/// Extract the file path portion from a /r/subdomain/path URL
/// Returns the path after the subdomain portion (e.g., "/r/abc123/foo/bar" -> "/foo/bar")
pub fn get_file_path_from_url(path: &str) -> String {
    let path_lower = path.to_lowercase();
    let trimmed = path_lower.trim_start_matches('/');

    if !trimmed.starts_with("r/") {
        return path.to_string();
    }

    // Skip "r/" + subdomain length
    let after_r = trimmed[2..].trim_start_matches('/');
    let after_subdomain = after_r
        .chars()
        .skip(CONFIG.subdomain_length)
        .collect::<String>();
    let trimmed_path = after_subdomain.trim_start_matches('/');

    if trimmed_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed_path}")
    }
}

pub fn get_subdomain_from_hostname(host: &str) -> Option<String> {
    if host.is_empty() {
        return None;
    }

    let host = host.to_lowercase();
    let domain = &CONFIG.server_domain;

    let r_index = host.rfind(domain)?;
    if r_index < CONFIG.subdomain_length + 1 {
        return None;
    }

    let subdomain = &host[r_index - 1 - CONFIG.subdomain_length..r_index - 1];

    if subdomain.is_empty()
        || subdomain.len() != CONFIG.subdomain_length
        || !subdomain
            .chars()
            .all(|c| CONFIG.subdomain_alphabet_set.contains(&c))
    {
        return None;
    }

    Some(subdomain.to_string())
}

pub fn generate_jwt(subdomain: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs() as i64;

    let claims = Claims {
        iat: now,
        exp: now + 60 * 60 * 24 * 365, // 1 year
        subdomain: subdomain.to_string(),
    };

    let header = Header::default();
    let key = EncodingKey::from_secret(CONFIG.jwt_secret.as_bytes());

    encode(&header, &claims, &key)
}

pub fn generate_share_jwt(
    request_id: &str,
    subdomain: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs() as i64;

    let claims = ShareClaims {
        iat: now,
        exp: now + 60 * 60 * 24 * 30, // 30 days
        request_id: request_id.to_string(),
        subdomain: subdomain.to_string(),
    };

    let header = Header::default();
    let key = EncodingKey::from_secret(CONFIG.jwt_secret.as_bytes());

    encode(&header, &claims, &key)
}

pub fn verify_share_jwt(token: &str) -> Option<ShareClaims> {
    let validation = Validation::default();
    let key = DecodingKey::from_secret(CONFIG.jwt_secret.as_bytes());

    match decode::<ShareClaims>(token, &key, &validation) {
        Ok(token_data) => Some(token_data.claims),
        Err(_) => None,
    }
}

pub async fn write_basic_file(subdomain: &str, cache: &crate::cache::Cache) -> anyhow::Result<()> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    let mut headers = vec![
        crate::models::Header {
            header: "Access-Control-Allow-Origin".to_string(),
            value: "*".to_string(),
        },
        crate::models::Header {
            header: "Content-Type".to_string(),
            value: "text/html; charset=utf-8".to_string(),
        },
    ];

    if CONFIG.include_server_domain {
        headers.push(crate::models::Header {
            header: "Server".to_string(),
            value: CONFIG.server_domain.clone(),
        });
    }

    // Create default index.html content
    let default_html = "Request logged successfully.";

    let file_data = crate::models::Response {
        raw: BASE64.encode(default_html),
        headers,
        status_code: 200,
    };

    let mut tree = std::collections::HashMap::new();
    tree.insert("index.html".to_string(), file_data);

    let tree_json = serde_json::to_string(&tree)?;
    cache.set(&format!("files:{subdomain}"), &tree_json).await?;

    Ok(())
}

pub fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs() as i64
}
