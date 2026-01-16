use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::seq::SliceRandom;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{error, info};
use trust_dns_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use trust_dns_proto::rr::rdata::{CNAME, MX, TXT};
use trust_dns_proto::rr::{Name, RData, Record, RecordType};
use trust_dns_proto::serialize::binary::{BinDecodable, BinEncodable};

use crate::cache::Cache;
use crate::ip2country::lookup_country;
use crate::models::{CacheMessage, DnsRequestLog};
use crate::utils::config::CONFIG;
use crate::utils::{generate_request_id, get_current_timestamp, get_subdomain_from_hostname};

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
}

impl Server {
    pub fn new(cache: Arc<Cache>, tx: Arc<broadcast::Sender<CacheMessage>>) -> Self {
        Self { cache, tx }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting DNS server on port {}", CONFIG.dns_port);

        let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", CONFIG.dns_port)).await?);
        let mut buf = vec![0u8; 512];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let cache = self.cache.clone();
                    let tx = self.tx.clone();
                    let socket_clone = socket.clone();

                    // Spawn a task to handle the request
                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_dns_request(&data, addr, cache, tx, socket_clone).await
                        {
                            error!("Error handling DNS request: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error receiving DNS packet: {}", e);
                }
            }
        }
    }
}

async fn handle_dns_request(
    data: &[u8],
    addr: SocketAddr,
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    let request =
        Message::from_bytes(data).map_err(|e| anyhow!("Failed to parse DNS request: {}", e))?;

    let query = match request.queries().first() {
        Some(q) => q,
        None => return Ok(()), // No query to process
    };

    let name = query.name().to_string();

    // Extract subdomain from hostname
    let subdomain = get_subdomain_from_hostname(&name);

    // Build response first so we can include it in the log
    let response = build_dns_response(&request, query, &subdomain, &cache).await?;

    // Format the response for logging
    let reply = format_dns_response(&response);

    // Log the DNS request if we have a valid subdomain
    if let Some(ref subdomain) = subdomain {
        if let Err(e) = log_dns_request(&request, data, addr, subdomain, &reply, &cache, &tx).await
        {
            error!("Failed to log DNS request: {}", e);
        }
    }

    // Serialize and send response
    let response_bytes = response
        .to_bytes()
        .map_err(|e| anyhow!("Failed to serialize DNS response: {}", e))?;
    socket.send_to(&response_bytes, addr).await?;

    Ok(())
}

/// Format a DNS response message for human-readable display
fn format_dns_response(response: &Message) -> String {
    let mut output = Vec::new();

    // Header section
    output.push(format!(
        ";; ->>HEADER<<- opcode: {:?}, status: {:?}, id: {}",
        response.op_code(),
        response.response_code(),
        response.id()
    ));

    let answers: Vec<_> = response.answers().iter().collect();
    let queries: Vec<_> = response.queries().iter().collect();

    let flags = format!(
        ";; flags: {}{}{}{}; QUERY: {}, ANSWER: {}, AUTHORITY: {}, ADDITIONAL: {}",
        if response.recursion_desired() {
            "qr "
        } else {
            ""
        },
        if response.authoritative() { "aa " } else { "" },
        if response.truncated() { "tc " } else { "" },
        if response.recursion_available() {
            "rd "
        } else {
            ""
        },
        queries.len(),
        answers.len(),
        response.name_server_count(),
        response.additional_count()
    );
    output.push(flags);

    // Question section
    output.push(";; QUESTION SECTION:".to_string());
    for query in &queries {
        output.push(format!(";{} IN {:?}", query.name(), query.query_type()));
    }

    // Answer section
    if !answers.is_empty() {
        output.push(";; ANSWER SECTION:".to_string());
        for answer in &answers {
            let rdata = match answer.data() {
                Some(data) => format!("{data}"),
                None => "".to_string(),
            };
            output.push(format!(
                "{} {} IN {:?} {}",
                answer.name(),
                answer.ttl(),
                answer.record_type(),
                rdata
            ));
        }
    }

    output.join("\n")
}

async fn log_dns_request(
    request: &Message,
    raw_data: &[u8],
    addr: SocketAddr,
    subdomain: &str,
    reply: &str,
    cache: &Arc<Cache>,
    tx: &Arc<broadcast::Sender<CacheMessage>>,
) -> Result<()> {
    let query = request
        .queries()
        .first()
        .ok_or_else(|| anyhow!("No query in request"))?;
    let name = query.name().to_string();
    let query_type = query.query_type();
    let source_ip = addr.ip().to_string();

    let request_id = generate_request_id();
    let country = lookup_country(&source_ip);

    let request_log = DnsRequestLog {
        _id: request_id.clone(),
        r#type: "dns".to_string(),
        raw: BASE64.encode(raw_data),
        uid: subdomain.to_string(),
        query_type: format!("{query_type:?}"),
        domain: name,
        date: get_current_timestamp(),
        ip: Some(source_ip),
        port: Some(addr.port()),
        country,
        reply: Some(reply.to_string()),
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

/// Check if the query is for our domain (including base domain and subdomains)
fn is_our_domain(name_str: &str) -> bool {
    let name_lower = name_str.to_lowercase();
    let domain = CONFIG.server_domain.to_lowercase();
    let domain_with_dot = format!("{domain}.");

    // Check if it's exactly our domain or a subdomain of it
    name_lower == domain
        || name_lower == domain_with_dot
        || name_lower.ends_with(&format!(".{domain}"))
        || name_lower.ends_with(&format!(".{domain_with_dot}"))
}

async fn build_dns_response(
    request: &Message,
    query: &Query,
    _subdomain: &Option<String>,
    cache: &Arc<Cache>,
) -> Result<Message> {
    let name = query.name().clone();
    let name_str = name.to_string();
    let query_type = query.query_type();

    let mut response = Message::new();
    response.set_id(request.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(OpCode::Query);
    response.set_recursion_desired(request.recursion_desired());
    response.set_recursion_available(true);
    response.set_authoritative(true);
    response.add_query(query.clone());

    // If query is not for our domain at all, return NXDOMAIN
    if !is_our_domain(&name_str) {
        response.set_response_code(ResponseCode::NXDomain);
        return Ok(response);
    }

    // Note: We handle queries even without a valid session subdomain
    // This allows ACME challenges and base domain queries to work

    // DNS is case-insensitive per RFC 1035 - normalize to lowercase for all lookups
    let name_lower = name_str.to_lowercase();

    match query_type {
        RecordType::A => {
            let dns_key = format!("dns:A:{name_lower}");
            let custom_record = cache.get(&dns_key).await.unwrap_or(None);

            if let Some(value) = custom_record {
                // Support multiple IPs separated by '%' - pick one randomly
                let ips: Vec<&str> = value.split('%').collect();
                let selected_ip = ips.choose(&mut rand::thread_rng()).unwrap_or(&ips[0]);

                if let Ok(ip) = selected_ip.parse::<Ipv4Addr>() {
                    let record = Record::from_rdata(name.clone(), 1, RData::A(ip.into()));
                    response.add_answer(record);
                }
            } else {
                // Default to server IP
                let ip = Ipv4Addr::from_str(&CONFIG.server_ip)
                    .unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1));
                let record = Record::from_rdata(name.clone(), 1, RData::A(ip.into()));
                response.add_answer(record);
            }
            response.set_response_code(ResponseCode::NoError);
        }
        RecordType::AAAA => {
            let dns_key = format!("dns:AAAA:{name_lower}");
            let custom_record = cache.get(&dns_key).await.unwrap_or(None);

            if let Some(value) = custom_record {
                // Support multiple IPs separated by '%' - pick one randomly
                let ips: Vec<&str> = value.split('%').collect();
                let selected_ip = ips.choose(&mut rand::thread_rng()).unwrap_or(&ips[0]);

                if let Ok(ip) = selected_ip.parse::<Ipv6Addr>() {
                    let record = Record::from_rdata(name.clone(), 1, RData::AAAA(ip.into()));
                    response.add_answer(record);
                    response.set_response_code(ResponseCode::NoError);
                } else {
                    response.set_response_code(ResponseCode::NXDomain);
                }
            } else {
                // Default: try to parse server_ip as IPv6, otherwise return NXDomain
                if let Ok(ip) = CONFIG.server_ip.parse::<Ipv6Addr>() {
                    let record = Record::from_rdata(name.clone(), 1, RData::AAAA(ip.into()));
                    response.add_answer(record);
                    response.set_response_code(ResponseCode::NoError);
                } else {
                    // server_ip is IPv4, no default AAAA available
                    response.set_response_code(ResponseCode::NoError);
                }
            }
        }
        RecordType::CNAME => {
            let dns_key = format!("dns:CNAME:{name_lower}");
            let custom_record = cache.get(&dns_key).await.unwrap_or(None);

            let cname_value = custom_record.unwrap_or_else(|| {
                // Default to server_domain with trailing dot
                let domain = &CONFIG.server_domain;
                if domain.ends_with('.') {
                    domain.clone()
                } else {
                    format!("{domain}.")
                }
            });

            if let Ok(target) = Name::from_str(&cname_value) {
                let record = Record::from_rdata(name.clone(), 1, RData::CNAME(CNAME(target)));
                response.add_answer(record);
                response.set_response_code(ResponseCode::NoError);
            } else {
                response.set_response_code(ResponseCode::NXDomain);
            }
        }
        RecordType::TXT => {
            let dns_key = format!("dns:TXT:{name_lower}");
            let custom_record = cache.get(&dns_key).await.unwrap_or(None);

            let txt_value = custom_record.unwrap_or_else(|| CONFIG.txt_record.clone());
            let txt_data = TXT::new(vec![txt_value]);
            let record = Record::from_rdata(name.clone(), 1, RData::TXT(txt_data));
            response.add_answer(record);
            response.set_response_code(ResponseCode::NoError);
        }
        RecordType::MX => {
            // Return the server domain as the mail exchange
            let mx_target = if CONFIG.server_domain.ends_with('.') {
                CONFIG.server_domain.clone()
            } else {
                format!("{}.", CONFIG.server_domain)
            };

            if let Ok(exchange) = Name::from_str(&mx_target) {
                // Priority 10, pointing to our server
                let mx_data = MX::new(10, exchange);
                let record = Record::from_rdata(name.clone(), 300, RData::MX(mx_data));
                response.add_answer(record);
                response.set_response_code(ResponseCode::NoError);
            } else {
                response.set_response_code(ResponseCode::ServFail);
            }
        }
        _ => {
            response.set_response_code(ResponseCode::NXDomain);
        }
    }

    Ok(response)
}
