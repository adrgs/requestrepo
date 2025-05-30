
use anyhow::{anyhow, Result};
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

async fn handle_smtp_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
) -> Result<()> {
    let subdomain = crate::utils::get_random_subdomain();
    
    socket.write_all(format!("220 {} ESMTP RequestRepo\r\n", CONFIG.server_domain).as_bytes()).await?;
    
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    
    let mut line = String::new();
    
    let mut current_command = String::new();
    let mut data_mode = false;
    let mut email_data = String::new();
    
    let client_ip = addr.ip().to_string();
    
    while reader.read_line(&mut line).await? > 0 {
        let line_trimmed = line.trim();
        
        if data_mode {
            if line_trimmed == "." {
                data_mode = false;
                
                log_smtp_request(
                    &subdomain,
                    "DATA",
                    Some(&email_data),
                    &client_ip,
                    &cache,
                    &tx,
                ).await?;
                
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
        
        current_command = line_trimmed.to_string();
        
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
        raw: base64::encode(format!("Command: {}\nData: {:?}", command, data)),
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
