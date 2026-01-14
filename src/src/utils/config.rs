
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::env;

pub struct Config {
    pub server_ip: String,
    pub server_domain: String,
    pub include_server_domain: bool,
    pub subdomain_length: usize,
    pub subdomain_alphabet: String,
    pub subdomain_alphabet_set: HashSet<char>,
    pub jwt_secret: String,
    pub txt_record: String,
    pub http_port: u16,
    pub dns_port: u16,
    pub smtp_port: u16,
    pub tcp_port_range_start: u16,
    pub tcp_port_range_end: u16,
    // New: ADMIN_TOKEN for org-level access control
    pub admin_token: Option<String>,
    // New: Per-subdomain storage limit (files + DNS)
    pub max_subdomain_size_bytes: usize,
    // New: Max HTTP request body size
    pub max_request_body_bytes: usize,
    // New: Max cache memory as percentage of container limit
    pub cache_max_memory_pct: f64,
    // TLS/ACME configuration
    pub tls_enabled: bool,
    pub https_port: u16,
    pub cert_dir: String,
    pub acme_email: Option<String>,
    pub acme_directory: String,
    pub cert_renewal_days: u64,
    pub cert_check_hours: u64,
    // Rate limiting for session creation
    pub session_rate_limit: u32,
    pub session_rate_window_secs: u64,
}

impl Config {
    pub fn new() -> Self {
        let server_ip = env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_domain = env::var("DOMAIN").unwrap_or_else(|_| "localhost".to_string()).to_lowercase();
        let include_server_domain = env::var("INCLUDE_SERVER_DOMAIN")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        let subdomain_length = env::var("SUBDOMAIN_LENGTH")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .unwrap_or(8);
        let subdomain_alphabet = env::var("SUBDOMAIN_ALPHABET")
            .unwrap_or_else(|_| "0123456789abcdefghijklmnopqrstuvwxyz".to_string());
        let subdomain_alphabet_set = subdomain_alphabet.chars().collect();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        let txt_record = env::var("TXT").unwrap_or_else(|_| "Hello!".to_string());
        let http_port = env::var("HTTP_PORT")
            .unwrap_or_else(|_| "21337".to_string())
            .parse()
            .unwrap_or(21337);
        let dns_port = env::var("DNS_PORT")
            .unwrap_or_else(|_| "53".to_string())
            .parse()
            .unwrap_or(53);
        let smtp_port = env::var("SMTP_PORT")
            .unwrap_or_else(|_| "25".to_string())
            .parse()
            .unwrap_or(25);
        let tcp_port_range_start = env::var("TCP_PORT_RANGE_START")
            .unwrap_or_else(|_| "10000".to_string())
            .parse()
            .unwrap_or(10000);
        let tcp_port_range_end = env::var("TCP_PORT_RANGE_END")
            .unwrap_or_else(|_| "11000".to_string())
            .parse()
            .unwrap_or(11000);

        // ADMIN_TOKEN: If set, requires auth for API access (except catch-all)
        let admin_token = env::var("ADMIN_TOKEN").ok().filter(|s| !s.is_empty());

        // Per-subdomain storage limit: default 10MB
        let max_subdomain_size_bytes = env::var("MAX_SUBDOMAIN_SIZE_MB")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<usize>()
            .unwrap_or(10)
            * 1024 * 1024;

        // Max HTTP request body size: default 10MB
        let max_request_body_bytes = env::var("MAX_REQUEST_BODY_MB")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<usize>()
            .unwrap_or(10)
            * 1024 * 1024;

        // Max cache memory as percentage of container limit: default 70%
        let cache_max_memory_pct = env::var("CACHE_MAX_MEMORY_PCT")
            .unwrap_or_else(|_| "0.7".to_string())
            .parse()
            .unwrap_or(0.7);

        // TLS/ACME configuration
        let tls_enabled = env::var("TLS_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        let https_port = env::var("HTTPS_PORT")
            .unwrap_or_else(|_| "443".to_string())
            .parse()
            .unwrap_or(443);
        let cert_dir = env::var("CERT_DIR")
            .unwrap_or_else(|_| "/app/certs".to_string());
        let acme_email = env::var("ACME_EMAIL").ok().filter(|s| !s.is_empty());
        let acme_directory = env::var("ACME_DIRECTORY")
            .unwrap_or_else(|_| "https://acme-v02.api.letsencrypt.org/directory".to_string());
        let cert_renewal_days = env::var("CERT_RENEWAL_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse()
            .unwrap_or(7);
        let cert_check_hours = env::var("CERT_CHECK_HOURS")
            .unwrap_or_else(|_| "12".to_string())
            .parse()
            .unwrap_or(12);

        // Rate limiting for session creation: max requests per IP per window
        let session_rate_limit = env::var("SESSION_RATE_LIMIT")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .unwrap_or(10);
        let session_rate_window_secs = env::var("SESSION_RATE_WINDOW_SECS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60);

        Self {
            server_ip,
            server_domain,
            include_server_domain,
            subdomain_length,
            subdomain_alphabet,
            subdomain_alphabet_set,
            jwt_secret,
            txt_record,
            http_port,
            dns_port,
            smtp_port,
            tcp_port_range_start,
            tcp_port_range_end,
            admin_token,
            max_subdomain_size_bytes,
            max_request_body_bytes,
            cache_max_memory_pct,
            tls_enabled,
            https_port,
            cert_dir,
            acme_email,
            acme_directory,
            cert_renewal_days,
            cert_check_hours,
            session_rate_limit,
            session_rate_window_secs,
        }
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}
