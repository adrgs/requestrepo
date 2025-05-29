
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::env;

pub struct Config {
    pub redis_host: String,
    pub server_ip: String,
    pub server_domain: String,
    pub include_server_domain: bool,
    pub subdomain_length: usize,
    pub subdomain_alphabet: String,
    pub subdomain_alphabet_set: HashSet<char>,
    pub jwt_secret: String,
    pub max_file_size: usize,
    pub max_request_size: usize,
    pub txt_record: String,
    pub cache_ttl_days: u64,
    pub http_port: u16,
    pub https_port: u16,
    pub dns_port: u16,
    pub smtp_port: u16,
    pub tcp_port_range_start: u16,
    pub tcp_port_range_end: u16,
    pub cert_path: String,
}

impl Config {
    pub fn new() -> Self {
        let redis_host = env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
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
        let max_file_size = env::var("MAX_FILE_SIZE")
            .unwrap_or_else(|_| (1024 * 1024 * 2).to_string())
            .parse()
            .unwrap_or(1024 * 1024 * 2);
        let max_request_size = env::var("MAX_REQUEST_SIZE")
            .unwrap_or_else(|_| (1024 * 1024 * 10).to_string())
            .parse()
            .unwrap_or(1024 * 1024 * 10);
        let txt_record = env::var("TXT").unwrap_or_else(|_| "Hello!".to_string());
        let cache_ttl_days = env::var("REDIS_TTL_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse()
            .unwrap_or(7);
        let http_port = env::var("HTTP_PORT")
            .unwrap_or_else(|_| "8001".to_string())
            .parse()
            .unwrap_or(8001);
        let https_port = env::var("HTTPS_PORT")
            .unwrap_or_else(|_| "8443".to_string())
            .parse()
            .unwrap_or(8443);
        let dns_port = env::var("DNS_PORT")
            .unwrap_or_else(|_| "5353".to_string())
            .parse()
            .unwrap_or(5353);
        let smtp_port = env::var("SMTP_PORT")
            .unwrap_or_else(|_| "2525".to_string())
            .parse()
            .unwrap_or(2525);
        let tcp_port_range_start = env::var("TCP_PORT_RANGE_START")
            .unwrap_or_else(|_| "10000".to_string())
            .parse()
            .unwrap_or(10000);
        let tcp_port_range_end = env::var("TCP_PORT_RANGE_END")
            .unwrap_or_else(|_| "50000".to_string())
            .parse()
            .unwrap_or(50000);
        let cert_path = env::var("CERT_PATH").unwrap_or_else(|_| "./certs/".to_string());

        Self {
            redis_host,
            server_ip,
            server_domain,
            include_server_domain,
            subdomain_length,
            subdomain_alphabet,
            subdomain_alphabet_set,
            jwt_secret,
            max_file_size,
            max_request_size,
            txt_record,
            cache_ttl_days,
            http_port,
            https_port,
            dns_port,
            smtp_port,
            tcp_port_range_start,
            tcp_port_range_end,
            cert_path,
        }
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}
