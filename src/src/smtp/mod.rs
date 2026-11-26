use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use mail_parser::{MessageParser, MimeHeaders};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Semaphore};
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::abuse::AbuseList;
use crate::cache::Cache;
use crate::ip2country::lookup_country;
use crate::models::{CacheMessage, SmtpAttachment, SmtpRequestLog};
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

/// Maximum transaction log size (64 KB)
const MAX_TRANSACTION_LOG_SIZE: usize = 64 * 1024;

/// Maximum commands per session
const MAX_COMMANDS_PER_SESSION: u32 = 500;

/// Maximum recipients per session
const MAX_RECIPIENTS: usize = 100;

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    abuse: Arc<AbuseList>,
}

impl Server {
    pub fn new(
        cache: Arc<Cache>,
        tx: Arc<broadcast::Sender<CacheMessage>>,
        abuse: Arc<AbuseList>,
    ) -> Self {
        Self { cache, tx, abuse }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting SMTP server on port {}", CONFIG.smtp_port);

        let listener = TcpListener::bind(format!("0.0.0.0:{}", CONFIG.smtp_port)).await?;
        let semaphore = Arc::new(Semaphore::new(100));

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let cache = self.cache.clone();
                    let tx = self.tx.clone();
                    let abuse = self.abuse.clone();

                    match semaphore.clone().try_acquire_owned() {
                        Ok(permit) => {
                            tokio::spawn(async move {
                                let _permit = permit;
                                // Apply overall connection timeout
                                match timeout(
                                    CONNECTION_TIMEOUT,
                                    handle_smtp_connection(socket, addr, cache, tx, abuse),
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
                        Err(_) => {
                            warn!(
                                "SMTP server at capacity, rejecting connection from {}",
                                addr
                            );
                            drop(socket);
                        }
                    }
                }
                Err(e) => {
                    error!("Error accepting SMTP connection: {}", e);
                }
            }
        }
    }
}

/// Read a line from the reader with a maximum length limit.
/// Unlike `read_line()`, this enforces the limit DURING the read,
/// preventing unbounded memory allocation from newline-free input.
async fn read_line_limited<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut String,
    max_len: usize,
) -> std::io::Result<usize> {
    let mut total = 0;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(total); // EOF
        }
        let remaining = (max_len + 1).saturating_sub(total);
        if remaining == 0 {
            return Ok(total);
        }
        let to_scan = available.len().min(remaining);
        if let Some(pos) = available[..to_scan].iter().position(|&b| b == b'\n') {
            let to_consume = pos + 1;
            buf.push_str(&String::from_utf8_lossy(&available[..to_consume]));
            reader.consume(to_consume);
            total += to_consume;
            return Ok(total);
        }
        buf.push_str(&String::from_utf8_lossy(&available[..to_scan]));
        reader.consume(to_scan);
        total += to_scan;
        if total > max_len {
            return Ok(total); // Hit limit, caller will disconnect
        }
    }
}

/// The subdomain an address belongs to, unless that subdomain is blocked.
fn recipient_subdomain(email: &str, abuse: &AbuseList) -> Option<String> {
    let subdomain = extract_subdomain_from_email(email)?;
    (!abuse.is_blocked(&subdomain)).then_some(subdomain)
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
    abuse: Arc<AbuseList>,
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
    let mut transaction_log = String::new();
    let mut command_count: u32 = 0;

    // Log the greeting we sent
    transaction_log.push_str(&format!(
        "S: 220 {} ESMTP RequestRepo\n",
        CONFIG.server_domain
    ));

    loop {
        line.clear();

        // Read with timeout, enforcing max line length during the read
        let read_result = timeout(
            READ_TIMEOUT,
            read_line_limited(&mut reader, &mut line, MAX_LINE_LENGTH),
        )
        .await;

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

        // Line length is bounded by read_line_limited, but check for overlength
        if line.len() > MAX_LINE_LENGTH {
            let _ = writer.write_all(b"500 Line too long\r\n").await;
            break;
        }

        let line_trimmed = line.trim();

        // Check transaction log size
        if transaction_log.len() > MAX_TRANSACTION_LOG_SIZE {
            let _ = writer
                .write_all(b"421 Transaction log too large - disconnecting\r\n")
                .await;
            break;
        }

        // Check command count
        command_count += 1;
        if command_count > MAX_COMMANDS_PER_SESSION {
            let _ = writer
                .write_all(b"421 Too many commands - disconnecting\r\n")
                .await;
            break;
        }

        if data_mode {
            // In data mode, only strip trailing whitespace to preserve leading
            // whitespace needed for MIME header folding (RFC 2822 continuation lines)
            let data_line = line.trim_end();
            if data_line == "." {
                data_mode = false;
                transaction_log.push_str("C: .\n");
                transaction_log.push_str("S: 250 OK: Message received\n");

                // If we don't have a subdomain yet, try to extract from email headers (To, CC, BCC)
                if subdomain.is_none() {
                    let all_recipients = extract_all_recipients(&email_data);
                    for recipient in all_recipients {
                        if let Some(extracted) = recipient_subdomain(&recipient, &abuse) {
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
                        &transaction_log,
                        &cache,
                        &tx,
                    )
                    .await?;
                }

                email_data.clear();
                writer.write_all(b"250 OK: Message received\r\n").await?;
            } else {
                // Check message size
                if email_data.len() + data_line.len() > MAX_MESSAGE_SIZE {
                    data_mode = false;
                    transaction_log.push_str("S: 552 Message size exceeds maximum\n");
                    writer
                        .write_all(b"552 Message size exceeds maximum\r\n")
                        .await?;
                    continue;
                }

                // Handle dot-stuffing (lines starting with . have the . removed)
                // Preserve \r\n line endings for correct MIME parsing
                let content = data_line.strip_prefix('.').unwrap_or(data_line);
                email_data.push_str(content);
                email_data.push_str("\r\n");
            }
        } else {
            if line_trimmed.is_empty() {
                continue;
            }

            // Log client command
            transaction_log.push_str(&format!("C: {line_trimmed}\n"));

            let parts: Vec<&str> = line_trimmed.splitn(2, ' ').collect();
            let command = parts[0].to_uppercase();
            let args = parts.get(1).map(|s| s.to_string());

            match command.as_str() {
                "HELO" => {
                    let response = format!("250 {} Hello", CONFIG.server_domain);
                    transaction_log.push_str(&format!("S: {response}\n"));
                    writer
                        .write_all(format!("{response}\r\n").as_bytes())
                        .await?;
                }
                "EHLO" => {
                    let response = format!(
                        "250-{} Hello\n250-SIZE {}\n250-8BITMIME\n250 HELP",
                        CONFIG.server_domain, MAX_MESSAGE_SIZE
                    );
                    transaction_log.push_str(&format!("S: {response}\n"));
                    let wire_response = format!(
                        "250-{} Hello\r\n250-SIZE {}\r\n250-8BITMIME\r\n250 HELP\r\n",
                        CONFIG.server_domain, MAX_MESSAGE_SIZE
                    );
                    writer.write_all(wire_response.as_bytes()).await?;
                }
                "MAIL" => {
                    // Parse MAIL FROM:<address>
                    if let Some(ref arg) = args {
                        let arg_upper = arg.to_uppercase();
                        if arg_upper.starts_with("FROM:") {
                            mail_from = Some(arg[5..].trim().to_string());
                            transaction_log.push_str("S: 250 OK\n");
                            writer.write_all(b"250 OK\r\n").await?;
                        } else {
                            transaction_log.push_str("S: 501 Syntax error\n");
                            writer.write_all(b"501 Syntax error\r\n").await?;
                        }
                    } else {
                        transaction_log.push_str("S: 501 Syntax error\n");
                        writer.write_all(b"501 Syntax error\r\n").await?;
                    }
                }
                "RCPT" => {
                    // Parse RCPT TO:<address>
                    if let Some(ref arg) = args {
                        let arg_upper = arg.to_uppercase();
                        if arg_upper.starts_with("TO:") {
                            // Check recipient limit
                            if rcpt_to.len() >= MAX_RECIPIENTS {
                                transaction_log.push_str("S: 452 Too many recipients\n");
                                writer.write_all(b"452 Too many recipients\r\n").await?;
                                continue;
                            }

                            let recipient = arg[3..].trim().to_string();

                            // Refuse mail for a blocked subdomain at the envelope, so that
                            // it is neither accepted nor logged
                            if let Some(extracted) = extract_subdomain_from_email(&recipient) {
                                if abuse.is_blocked(&extracted) {
                                    transaction_log.push_str(
                                        "S: 550 5.7.1 Recipient address rejected: subdomain is suspended\n",
                                    );
                                    writer
                                        .write_all(
                                            b"550 5.7.1 Recipient address rejected: subdomain is suspended\r\n",
                                        )
                                        .await?;
                                    continue;
                                }
                            }

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
                            transaction_log.push_str("S: 250 OK\n");
                            writer.write_all(b"250 OK\r\n").await?;
                        } else {
                            transaction_log.push_str("S: 501 Syntax error\n");
                            writer.write_all(b"501 Syntax error\r\n").await?;
                        }
                    } else {
                        transaction_log.push_str("S: 501 Syntax error\n");
                        writer.write_all(b"501 Syntax error\r\n").await?;
                    }
                }
                "DATA" => {
                    if mail_from.is_none() {
                        transaction_log.push_str("S: 503 Need MAIL command first\n");
                        writer.write_all(b"503 Need MAIL command first\r\n").await?;
                    } else if rcpt_to.is_empty() {
                        transaction_log.push_str("S: 503 Need RCPT command first\n");
                        writer.write_all(b"503 Need RCPT command first\r\n").await?;
                    } else {
                        transaction_log
                            .push_str("S: 354 Start mail input; end with <CRLF>.<CRLF>\n");
                        transaction_log.push_str("[EMAIL DATA]\n");
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
                    transaction_log.push_str("S: 250 OK\n");
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "NOOP" => {
                    transaction_log.push_str("S: 250 OK\n");
                    writer.write_all(b"250 OK\r\n").await?;
                }
                "QUIT" => {
                    transaction_log.push_str("S: 221 Bye\n");
                    writer.write_all(b"221 Bye\r\n").await?;
                    break;
                }
                "VRFY" | "EXPN" => {
                    transaction_log
                        .push_str("S: 252 Cannot verify user, but will accept message\n");
                    writer
                        .write_all(b"252 Cannot verify user, but will accept message\r\n")
                        .await?;
                }
                "HELP" => {
                    transaction_log
                        .push_str("S: 214 RequestRepo SMTP server - https://requestrepo.com\n");
                    writer
                        .write_all(b"214 RequestRepo SMTP server - https://requestrepo.com\r\n")
                        .await?;
                }
                _ => {
                    transaction_log.push_str("S: 500 Command not recognized\n");
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
    text_body: Option<String>,
    html_body: Option<String>,
    attachments: Vec<SmtpAttachment>,
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

        let text_body = message.body_text(0).map(|s| s.to_string());
        let html_body = message.body_html(0).map(|s| s.to_string());

        let attachments: Vec<SmtpAttachment> = message
            .attachments()
            .filter(|part| {
                // Skip MIME container types — they're structure, not real attachments
                let ctype = part
                    .content_type()
                    .map(|ct| ct.ctype().to_string())
                    .unwrap_or_default();
                ctype != "multipart" && ctype != "message"
            })
            .filter(|part| {
                // Skip text/plain and text/html body parts (already in text_body/html_body)
                let is_inline_body = part.is_text() && part.attachment_name().is_none();
                !is_inline_body
            })
            .map(|part| {
                let content_type = part
                    .content_type()
                    .map(|ct| {
                        if let Some(subtype) = ct.subtype() {
                            format!("{}/{}", ct.ctype(), subtype)
                        } else {
                            ct.ctype().to_string()
                        }
                    })
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                SmtpAttachment {
                    filename: part.attachment_name().unwrap_or("unnamed").to_string(),
                    content_type,
                    size: part.contents().len(),
                    content: BASE64.encode(part.contents()),
                }
            })
            .collect();

        ParsedEmail {
            subject,
            from,
            to,
            cc,
            bcc,
            text_body,
            html_body,
            attachments,
        }
    } else {
        // Fallback if parsing fails
        ParsedEmail {
            subject: None,
            from: None,
            to: None,
            cc: None,
            bcc: None,
            text_body: None,
            html_body: None,
            attachments: Vec::new(),
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
    transaction_log: &str,
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
        text_body: None,
        html_body: None,
        attachments: Vec::new(),
    });

    // Use the full transaction log as raw content
    let raw_content = transaction_log.to_string();

    // Fall back to SMTP envelope values if email headers are missing
    let from = parsed.from.or_else(|| mail_from.map(|s| s.to_string()));
    let to = parsed.to.or_else(|| {
        if rcpt_to.is_empty() {
            None
        } else {
            Some(rcpt_to.join(", "))
        }
    });

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
        // Include parsed email headers (with envelope fallback)
        subject: parsed.subject,
        from,
        to,
        cc: parsed.cc,
        bcc: parsed.bcc,
        text_body: parsed.text_body,
        html_body: parsed.html_body,
        attachments: parsed.attachments,
    };

    let request_json = serde_json::to_string(&request_log)?;

    // Push request to list
    let list_key = format!("requests:{subdomain}");
    cache.rpush(&list_key, &request_json).await?;

    let message = CacheMessage {
        cmd: "new_request".to_string(),
        subdomain: subdomain.to_string(),
        data: request_json,
    };

    let _ = tx.send(message);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Runs one SMTP conversation against `handle_smtp_connection` over a real socket and
    /// returns everything the server said. The client sends its lines up front; the server
    /// reads them one at a time either way.
    async fn conversation(cache: Arc<Cache>, abuse: Arc<AbuseList>, lines: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, _) = broadcast::channel(16);
        let server = tokio::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            handle_smtp_connection(stream, peer, cache, Arc::new(tx), abuse)
                .await
                .ok();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let script: String = lines.iter().map(|line| format!("{line}\r\n")).collect();
        client.write_all(script.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap();

        let mut said = String::new();
        client.read_to_string(&mut said).await.unwrap();
        server.await.unwrap();
        said
    }

    async fn stored_requests(cache: &Cache, subdomain: &str) -> usize {
        cache.llen(&format!("requests:{subdomain}")).await.unwrap()
    }

    #[tokio::test]
    async fn mail_to_a_blocked_subdomain_is_refused_at_the_envelope() {
        let blocked = "aaaaaaaa";
        let allowed = "bbbbbbbb";
        // Kept alive for the test: the blocklist saves into it
        let _dir = tempfile::TempDir::new().unwrap();
        let abuse = Arc::new(
            AbuseList::load(_dir.path(), crate::abuse::Settings::default())
                .await
                .unwrap(),
        );
        abuse.suspend(blocked).await;
        let cache = Arc::new(Cache::new());
        let domain = &CONFIG.server_domain;

        let said = conversation(
            cache.clone(),
            abuse.clone(),
            vec![
                "EHLO tester".to_string(),
                "MAIL FROM:<attacker@example.org>".to_string(),
                format!("RCPT TO:<victim@{blocked}.{domain}>"),
                format!("RCPT TO:<victim@{allowed}.{domain}>"),
                "DATA".to_string(),
                format!("To: <someone@{blocked}.{domain}>"),
                "Subject: hello".to_string(),
                String::new(),
                "body".to_string(),
                ".".to_string(),
                "QUIT".to_string(),
            ],
        )
        .await;

        // One recipient refused, the other accepted, and the message delivered
        let refused: Vec<&str> = said
            .lines()
            .filter(|line| line.starts_with("550"))
            .collect();
        assert_eq!(
            refused,
            ["550 5.7.1 Recipient address rejected: subdomain is suspended"]
        );
        // Accepted for MAIL and for the one recipient that was allowed
        assert_eq!(said.matches("250 OK\r\n").count(), 2, "{said}");
        assert!(said.contains("250 OK: Message received"), "{said}");

        // Logged for the subdomain that was allowed to receive it, never for the blocked one
        assert_eq!(stored_requests(&cache, allowed).await, 1);
        assert_eq!(stored_requests(&cache, blocked).await, 0);
    }

    #[tokio::test]
    async fn mail_is_never_attributed_to_a_blocked_subdomain_from_its_headers() {
        let blocked = "aaaaaaaa";
        let _dir = tempfile::TempDir::new().unwrap();
        let abuse = Arc::new(
            AbuseList::load(_dir.path(), crate::abuse::Settings::default())
                .await
                .unwrap(),
        );
        abuse.suspend(blocked).await;
        let cache = Arc::new(Cache::new());
        let domain = &CONFIG.server_domain;

        // The envelope names no subdomain of ours; only the To: header does
        let said = conversation(
            cache.clone(),
            abuse,
            vec![
                "EHLO tester".to_string(),
                "MAIL FROM:<attacker@example.org>".to_string(),
                "RCPT TO:<someone@elsewhere.org>".to_string(),
                "DATA".to_string(),
                format!("To: <x@{blocked}.{domain}>"),
                "Subject: hi".to_string(),
                String::new(),
                "body".to_string(),
                ".".to_string(),
                "QUIT".to_string(),
            ],
        )
        .await;

        assert!(said.contains("250 OK: Message received"), "{said}");
        assert_eq!(stored_requests(&cache, blocked).await, 0);
    }
}
