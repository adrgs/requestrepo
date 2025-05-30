
use anyhow::{anyhow, Result};
use base64::Engine;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::cache::Cache;
use crate::models::{CacheMessage, SmtpRequestLog};
use crate::utils::config::CONFIG;
use crate::utils::{generate_request_id, get_current_timestamp};
use crate::ip2country::lookup_country;

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
}

impl Server {
    pub fn new(cache: Arc<Cache>, tx: Arc<broadcast::Sender<CacheMessage>>) -> Self {
        Self { cache, tx }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting SMTP server on port {}", CONFIG.smtp_port);

        let listener = TcpListener::bind(format!("0.0.0.0:{}", CONFIG.smtp_port)).await?;

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let cache = self.cache.clone();
                    let tx = self.tx.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_smtp_connection(socket, addr, cache, tx).await {
                            error!("Error handling SMTP connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting SMTP connection: {}", e);
                }
            }
        }
    }
}

fn extract_subdomain_from_email(email: &str, _server_domain: &str) -> Option<String> {
    let email = email.trim_start_matches('<').trim_end_matches('>');
    
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let domain = parts[1];
    
    let domain_parts: Vec<&str> = domain.split('.').collect();
    if domain_parts.len() < 2 {
        return None;
    }
    
    let subdomain = domain_parts[0];
    
    if subdomain.len() != CONFIG.subdomain_length || 
       !subdomain.chars().all(|c| CONFIG.subdomain_alphabet_set.contains(&c)) {
        return None;
    }
    
    Some(subdomain.to_string())
}

fn extract_email_from_rcpt(rcpt_command: &str) -> Option<String> {
    let parts: Vec<&str> = rcpt_command.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    
    if !parts[0].eq_ignore_ascii_case("RCPT") || !parts[1].eq_ignore_ascii_case("TO:") {
        return None;
    }
    
    let email = if parts.len() > 2 {
        parts[2].trim()
    } else {
        return None;
    };
    
    Some(email.to_string())
}

async fn handle_smtp_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
) -> Result<()> {
    let mut subdomain = crate::utils::get_random_subdomain();
    let mut extracted_subdomains: Vec<String> = Vec::new();
    
    socket.write_all(format!("220 {} ESMTP RequestRepo\r\n", CONFIG.server_domain).as_bytes()).await?;
    
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    
    let mut line = String::new();
    
    let mut data_mode = false;
    let mut email_data = String::new();
    
    let client_ip = addr.ip().to_string();
    
    while reader.read_line(&mut line).await? > 0 {
        let line_trimmed = line.trim();
        
        if data_mode {
            if line_trimmed == "." {
                data_mode = false;
                
                if !extracted_subdomains.is_empty() {
                    for sub in &extracted_subdomains {
                        log_smtp_request(
                            sub,
                            "DATA",
                            Some(&email_data),
                            &client_ip,
                            &cache,
                            &tx,
                        ).await?;
                    }
                } else {
                    log_smtp_request(
                        &subdomain,
                        "DATA",
                        Some(&email_data),
                        &client_ip,
                        &cache,
                        &tx,
                    ).await?;
                }
                
                email_data.clear();
                
                writer.write_all(b"250 OK: Message received\r\n").await?;
            } else {
                email_data.push_str(line_trimmed);
                email_data.push('\n');
            }
        } else {
            if line_trimmed.is_empty() {
                line.clear();
                continue;
            }
            
            let parts: Vec<&str> = line_trimmed.splitn(2, ' ').collect();
            let command = parts[0].to_uppercase();
            
            if command == "RCPT" {
                if let Some(email) = extract_email_from_rcpt(line_trimmed) {
                    if let Some(extracted_subdomain) = extract_subdomain_from_email(&email, &CONFIG.server_domain) {
                        info!("Extracted subdomain from email: {}", extracted_subdomain);
                        extracted_subdomains.push(extracted_subdomain.clone());
                        subdomain = extracted_subdomain; // Use the last valid subdomain as default
                    }
                }
            }
            
            log_smtp_request(
                &subdomain,
                &command,
                None,
                &client_ip,
                &cache,
                &tx,
            ).await?;
            
            match command.as_str() {
                "HELO" | "EHLO" => {
                    writer.write_all(b"250-RequestRepo\r\n").await?;
                    writer.write_all(b"250-SIZE 10485760\r\n").await?;
                    writer.write_all(b"250 HELP\r\n").await?;
                }
                "MAIL" => {
                    extracted_subdomains.clear();
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "RCPT" => {
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "DATA" => {
                    writer.write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n").await?;
                    data_mode = true;
                }
                "QUIT" => {
                    writer.write_all(b"221 Bye\r\n").await?;
                    break;
                }
                _ => {
                    writer.write_all(b"500 Command not recognized\r\n").await?;
                }
            }
        }
        
        line.clear();
    }
    
    Ok(())
}

async fn log_smtp_request(
    subdomain: &str,
    command: &str,
    data: Option<&str>,
    client_ip: &str,
    cache: &Cache,
    tx: &broadcast::Sender<CacheMessage>,
) -> Result<()> {
    let request_id = generate_request_id();
    
    let country = lookup_country(client_ip);
    
    let request_log = SmtpRequestLog {
        _id: request_id.clone(),
        r#type: "smtp".to_string(),
        raw: base64::engine::general_purpose::STANDARD.encode(format!("Command: {}\nData: {:?}", command, data)),
        uid: subdomain.to_string(),
        command: command.to_string(),
        data: data.map(|s| s.to_string()),
        date: get_current_timestamp(),
        ip: Some(client_ip.to_string()),
        country,
    };
    
    let request_json = serde_json::to_string(&request_log)?;
    
    cache.rpush(&format!("requests:{}", subdomain), &request_json).await?;
    cache.set(&format!("request:{}:{}", subdomain, request_id), "0").await?;
    
    let message = CacheMessage {
        cmd: "new_request".to_string(),
        subdomain: subdomain.to_string(),
        data: request_json,
    };
    
    let _ = tx.send(message);
    
    Ok(())
}
