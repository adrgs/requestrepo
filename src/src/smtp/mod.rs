use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use mail_parser::MessageParser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::cache::Cache;
use crate::ip2country::lookup_country;
use crate::models::{CacheMessage, SmtpRequestLog};
use crate::utils::config::CONFIG;
use crate::utils::{generate_request_id, get_current_timestamp, get_subdomain_from_hostname};

/// Maximum size for email data (10 MB)
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Connection timeout (5 minutes)
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300);

/// Read timeout for individual commands (60 seconds)
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum line length (prevent memory exhaustion)
const MAX_LINE_LENGTH: usize = 16 * 1024;

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
                        // Apply overall connection timeout
                        match timeout(
                            CONNECTION_TIMEOUT,
                            handle_smtp_connection(socket, addr, cache, tx),
                        )
                        .await
                        {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => {
                                warn!("SMTP connection error from {}: {}", addr, e);
                            }
                            Err(_) => {
                                warn!("SMTP connection timeout from {}", addr);
                            }
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

/// Extract subdomain from an email address like "user@subdomain.domain.com"
fn extract_subdomain_from_email(email: &str) -> Option<String> {
    // Remove angle brackets if present: <user@domain> -> user@domain
    let email = email.trim_matches(|c| c == '<' || c == '>');

    // Split by @ to get domain part
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return None;
    }

    let domain = parts[1];
    get_subdomain_from_hostname(domain)
}

async fn handle_smtp_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
) -> Result<()> {
    let client_ip = addr.ip().to_string();

    info!("SMTP connection from {}", addr);

    // Send greeting
    let greeting = format!("220 {} ESMTP RequestRepo\r\n", CONFIG.server_domain);
    socket.write_all(greeting.as_bytes()).await?;

    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);

    let mut line = String::new();
    let mut data_mode = false;
    let mut email_data = String::new();
    let mut mail_from: Option<String> = None;
    let mut rcpt_to: Vec<String> = Vec::new();
    let mut subdomain: Option<String> = None;

    loop {
        line.clear();

        // Read with timeout
        let read_result = timeout(READ_TIMEOUT, reader.read_line(&mut line)).await;

        let bytes_read = match read_result {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                warn!("SMTP read error from {}: {}", addr, e);
                break;
            }
            Err(_) => {
                warn!("SMTP read timeout from {}", addr);
                let _ = writer.write_all(b"421 Connection timeout\r\n").await;
                break;
            }
        };

        if bytes_read == 0 {
            break;
        }

        // Check line length
        if line.len() > MAX_LINE_LENGTH {
            let _ = writer.write_all(b"500 Line too long\r\n").await;
            break;
        }

        let line_trimmed = line.trim();

        if data_mode {
            if line_trimmed == "." {
                data_mode = false;

                // If we don't have a subdomain yet, try to extract from email headers (To, CC, BCC)
                if subdomain.is_none() {
                    let all_recipients = extract_all_recipients(&email_data);
                    for recipient in all_recipients {
                        if let Some(extracted) = extract_subdomain_from_email(&recipient) {
                            info!(
                                "SMTP connection from {} matched subdomain from email headers: {}",
                                addr, extracted
                            );
                            subdomain = Some(extracted);
                            break;
                        }
                    }
                }

                // Log the complete email if we have a valid subdomain
                if let Some(ref sub) = subdomain {
                    log_smtp_request(
                        sub,
                        "DATA",
                        Some(&email_data),
                        &client_ip,
                        mail_from.as_deref(),
                        &rcpt_to,
                        &cache,
                        &tx,
                    )
                    .await?;
                }

                email_data.clear();

                writer.write_all(b"250 OK: Message received\r\n").await?;
            } else {
                // Check message size
                if email_data.len() + line.len() > MAX_MESSAGE_SIZE {
                    data_mode = false;
                    email_data.clear();
                    writer
                        .write_all(b"552 Message size exceeds maximum\r\n")
                        .await?;
                    continue;
                }

                // Handle dot-stuffing (lines starting with . have the . removed)
                let content = line_trimmed.strip_prefix('.').unwrap_or(line_trimmed);
                email_data.push_str(content);
                email_data.push('\n');
            }
        } else {
            if line_trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line_trimmed.splitn(2, ' ').collect();
            let command = parts[0].to_uppercase();
            let args = parts.get(1).map(|s| s.to_string());

            // We only log DATA commands (actual emails), not protocol commands like QUIT, EHLO, etc.

            match command.as_str() {
                "HELO" => {
                    let response = format!("250 {} Hello\r\n", CONFIG.server_domain);
                    writer.write_all(response.as_bytes()).await?;
                }
                "EHLO" => {
                    let responses = format!(
                        "250-{} Hello\r\n250-SIZE {}\r\n250-8BITMIME\r\n250 HELP\r\n",
                        CONFIG.server_domain, MAX_MESSAGE_SIZE
                    );
                    writer.write_all(responses.as_bytes()).await?;
                }
                "MAIL" => {
                    // Parse MAIL FROM:<address>
                    if let Some(ref arg) = args {
                        let arg_upper = arg.to_uppercase();
                        if arg_upper.starts_with("FROM:") {
                            mail_from = Some(arg[5..].trim().to_string());
                            writer.write_all(b"250 OK\r\n").await?;
                        } else {
                            writer.write_all(b"501 Syntax error\r\n").await?;
                        }
                    } else {
                        writer.write_all(b"501 Syntax error\r\n").await?;
                    }
                }
                "RCPT" => {
                    // Parse RCPT TO:<address>
                    if let Some(ref arg) = args {
                        let arg_upper = arg.to_uppercase();
                        if arg_upper.starts_with("TO:") {
                            let recipient = arg[3..].trim().to_string();

                            // Extract subdomain from the first valid recipient
                            if subdomain.is_none() {
                                if let Some(extracted) = extract_subdomain_from_email(&recipient) {
                                    info!(
                                        "SMTP connection from {} matched subdomain: {}",
                                        addr, extracted
                                    );
                                    subdomain = Some(extracted);
                                }
                            }

                            rcpt_to.push(recipient);
                            writer.write_all(b"250 OK\r\n").await?;
                        } else {
                            writer.write_all(b"501 Syntax error\r\n").await?;
                        }
                    } else {
                        writer.write_all(b"501 Syntax error\r\n").await?;
                    }
                }
                "DATA" => {
                    if mail_from.is_none() {
                        writer.write_all(b"503 Need MAIL command first\r\n").await?;
                    } else if rcpt_to.is_empty() {
                        writer.write_all(b"503 Need RCPT command first\r\n").await?;
                    } else {
                        writer
                            .write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n")
                            .await?;
                        data_mode = true;
                    }
                }
                "RSET" => {
                    mail_from = None;
                    rcpt_to.clear();
                    email_data.clear();
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "NOOP" => {
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "QUIT" => {
                    writer.write_all(b"221 Bye\r\n").await?;
                    break;
                }
                "VRFY" | "EXPN" => {
                    writer
                        .write_all(b"252 Cannot verify user, but will accept message\r\n")
                        .await?;
                }
                "HELP" => {
                    writer
                        .write_all(b"214 RequestRepo SMTP server - https://requestrepo.com\r\n")
                        .await?;
                }
                _ => {
                    writer.write_all(b"500 Command not recognized\r\n").await?;
                }
            }
        }
    }

    info!("SMTP connection closed from {}", addr);
    Ok(())
}

/// Parsed email headers for structured response
struct ParsedEmail {
    subject: Option<String>,
    from: Option<String>,
    to: Option<String>,
    cc: Option<String>,
    bcc: Option<String>,
}

/// Parse email data using mail-parser to extract headers
fn parse_email_headers(email_data: &str) -> ParsedEmail {
    let parser = MessageParser::default();

    if let Some(message) = parser.parse(email_data.as_bytes()) {
        // Extract Subject
        let subject = message.subject().map(|s| s.to_string());

        // Extract From (format as string)
        let from = message.from().and_then(|addrs| {
            addrs.first().map(|addr| {
                if let Some(name) = addr.name() {
                    if let Some(email) = addr.address() {
                        format!("{name} <{email}>")
                    } else {
                        name.to_string()
                    }
                } else {
                    addr.address().unwrap_or("").to_string()
                }
            })
        });

        // Extract To (format all recipients)
        let to = message.to().map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .collect::<Vec<_>>()
                .join(", ")
        });

        // Extract CC
        let cc = message.cc().map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .collect::<Vec<_>>()
                .join(", ")
        });

        // Extract BCC
        let bcc = message.bcc().map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .collect::<Vec<_>>()
                .join(", ")
        });

        ParsedEmail {
            subject,
            from,
            to,
            cc,
            bcc,
        }
    } else {
        // Fallback if parsing fails
        ParsedEmail {
            subject: None,
            from: None,
            to: None,
            cc: None,
            bcc: None,
        }
    }
}

/// Extract all email addresses from To, CC, and BCC headers
fn extract_all_recipients(email_data: &str) -> Vec<String> {
    let parser = MessageParser::default();
    let mut recipients = Vec::new();

    if let Some(message) = parser.parse(email_data.as_bytes()) {
        // Get To addresses
        if let Some(addrs) = message.to() {
            for addr in addrs.iter() {
                if let Some(email) = addr.address() {
                    recipients.push(email.to_string());
                }
            }
        }

        // Get CC addresses
        if let Some(addrs) = message.cc() {
            for addr in addrs.iter() {
                if let Some(email) = addr.address() {
                    recipients.push(email.to_string());
                }
            }
        }

        // Get BCC addresses
        if let Some(addrs) = message.bcc() {
            for addr in addrs.iter() {
                if let Some(email) = addr.address() {
                    recipients.push(email.to_string());
                }
            }
        }
    }

    recipients
}

#[allow(clippy::too_many_arguments)]
async fn log_smtp_request(
    subdomain: &str,
    command: &str,
    data: Option<&str>,
    client_ip: &str,
    mail_from: Option<&str>,
    rcpt_to: &[String],
    cache: &Cache,
    tx: &broadcast::Sender<CacheMessage>,
) -> Result<()> {
    let request_id = generate_request_id();
    let country = lookup_country(client_ip);

    // Parse email headers if we have data
    let parsed = data.map(parse_email_headers).unwrap_or(ParsedEmail {
        subject: None,
        from: None,
        to: None,
        cc: None,
        bcc: None,
    });

    // Build raw content for logging
    let raw_content = if let Some(email_data) = data {
        format!(
            "From: {}\nTo: {}\n\n{}",
            mail_from.unwrap_or("<unknown>"),
            rcpt_to.join(", "),
            email_data
        )
    } else {
        format!("Command: {command}")
    };

    let request_log = SmtpRequestLog {
        _id: request_id.clone(),
        r#type: "smtp".to_string(),
        raw: BASE64.encode(raw_content.as_bytes()),
        uid: subdomain.to_string(),
        command: command.to_string(),
        data: data.map(|s| s.to_string()),
        date: get_current_timestamp(),
        ip: Some(client_ip.to_string()),
        country,
        // Include parsed email headers
        subject: parsed.subject,
        from: parsed.from,
        to: parsed.to,
        cc: parsed.cc,
        bcc: parsed.bcc,
    };

    let request_json = serde_json::to_string(&request_log)?;

    // Push request to list and get the new length to calculate the correct index
    let list_key = format!("requests:{subdomain}");
    let index = cache
        .rpush(&list_key, &request_json)
        .await?
        .saturating_sub(1);

    // Store the index for this request ID (used by delete endpoint)
    cache
        .set(
            &format!("request:{subdomain}:{request_id}"),
            &index.to_string(),
        )
        .await?;

    let message = CacheMessage {
        cmd: "new_request".to_string(),
        subdomain: subdomain.to_string(),
        data: request_json,
    };

    let _ = tx.send(message);

    Ok(())
}
