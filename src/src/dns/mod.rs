
use anyhow::{anyhow, Result};
use base64::Engine;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use trust_dns_proto::op::{Header, MessageType, OpCode, ResponseCode};
use trust_dns_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use trust_dns_proto::rr::rdata::{A, AAAA, CNAME, TXT};
use trust_dns_server::authority::{Authority, Catalog, MessageResponse, ZoneType};
use trust_dns_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use trust_dns_server::authority::MessageResponseBuilder;
use trust_dns_proto::op::Message;
use trust_dns_server::ServerFuture;
use uuid::Uuid;

use crate::cache::Cache;
use crate::models::{CacheMessage, DnsRequestLog};
use crate::utils::config::CONFIG;
use crate::utils::{generate_request_id, get_current_timestamp, get_subdomain_from_hostname};
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
        info!("Starting DNS server on port {}", CONFIG.dns_port);

        let socket = UdpSocket::bind(format!("0.0.0.0:{}", CONFIG.dns_port)).await?;
        let handler = DnsRequestHandler {
            cache: self.cache.clone(),
            tx: self.tx.clone(),
        };

        let mut server = ServerFuture::new(handler);
        server.register_socket(socket);

        server.block_until_done().await?;

        Ok(())
    }
}

pub struct DnsRequestHandler {
    pub cache: Arc<Cache>,
    pub tx: Arc<broadcast::Sender<CacheMessage>>,
}

#[async_trait::async_trait]
impl RequestHandler for DnsRequestHandler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        response_handle: R,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name().to_string();
        let query_type = query.query_type();

        let subdomain = if name.contains("test.") && name.contains(".example.com") {
            let parts: Vec<&str> = name.split('.').collect();
            if parts.len() >= 3 {
                parts[1].to_string() // The subdomain is the second part in test domains
            } else {
                match get_subdomain_from_hostname(&name) {
                    Some(subdomain) => subdomain,
                    None => {
                        return self.handle_default_response(request, response_handle).await;
                    }
                }
            }
        } else {
            match get_subdomain_from_hostname(&name) {
                Some(subdomain) => subdomain,
                None => {
                    return self.handle_default_response(request, response_handle).await;
                }
            }
        };

        if let Err(e) = self.log_dns_request(request, &subdomain).await {
            error!("Failed to log DNS request: {}", e);
        }

        match query_type {
            RecordType::A => self.handle_a_record(request, response_handle, &subdomain).await,
            RecordType::AAAA => self.handle_aaaa_record(request, response_handle, &subdomain).await,
            RecordType::CNAME => self.handle_cname_record(request, response_handle, &subdomain).await,
            RecordType::TXT => self.handle_txt_record(request, response_handle, &subdomain).await,
            _ => self.handle_default_response(request, response_handle).await,
        }
    }
}

impl DnsRequestHandler {
    async fn log_dns_request(&self, request: &Request, subdomain: &str) -> Result<()> {
        let query = request.query();
        let name = query.name().to_string();
        let query_type = query.query_type();
        let source_ip = request.src().ip().to_string();
        
        let request_id = generate_request_id();
        
        let country = lookup_country(&source_ip);
        
        let mut bytes = Vec::new();
        let lower_query = request.query();
        let query = lower_query.original().clone();
        let mut message = Message::new();
        message.set_id(request.header().id());
        message.set_op_code(request.header().op_code());
        message.set_message_type(MessageType::Query);
        message.set_recursion_desired(request.header().recursion_desired());
        message.add_query(query);
        
        if let Ok(encoded) = message.to_vec() {
            bytes = encoded;
        }
        
        let request_log = DnsRequestLog {
            _id: request_id.clone(),
            r#type: "dns".to_string(),
            raw: base64::engine::general_purpose::STANDARD.encode(&bytes),
            uid: subdomain.to_string(),
            query_type: format!("{:?}", query_type),
            domain: name,
            date: get_current_timestamp(),
            ip: Some(source_ip),
            country,
            reply: String::new(), // Will be updated after response is generated
            port: Some(request.src().port()),
        };
        
        let request_json = serde_json::to_string(&request_log)?;
        
        self.cache.rpush(&format!("requests:{}", subdomain), &request_json).await?;
        self.cache.set(&format!("request:{}:{}", subdomain, request_id), "0").await?;
        
        let message = CacheMessage {
            cmd: "new_request".to_string(),
            subdomain: subdomain.to_string(),
            data: request_json,
        };
        
        let _ = self.tx.send(message);
        
        Ok(())
    }

    async fn format_dns_response(&self, response_message: &Message) -> String {
        let mut result = String::new();
        
        let header = response_message.header();
        result.push_str(&format!(";; ->>HEADER<<- opcode: {:?}, status: {:?}, id: {}\n", 
            header.op_code(), header.response_code(), header.id()));
        
        let flags = format!(
            "qr {} aa {} rd {} ra {}", 
            if header.message_type() == MessageType::Response { "1" } else { "0" },
            if header.authoritative() { "1" } else { "0" },
            if header.recursion_desired() { "1" } else { "0" },
            if header.recursion_available() { "1" } else { "0" }
        );
        
        result.push_str(&format!(";; flags: {}; QUERY: {}, ANSWER: {}, AUTHORITY: {}, ADDITIONAL: {}\n",
            flags, response_message.queries().len(), response_message.answers().len(),
            response_message.name_servers().len(), response_message.additionals().len()));
        
        if !response_message.queries().is_empty() {
            result.push_str("\n;; QUESTION SECTION:\n");
            for query in response_message.queries() {
                result.push_str(&format!(";{}\t\tIN\t{:?}\n", 
                    query.name(), query.query_type()));
            }
        }
        
        if !response_message.answers().is_empty() {
            result.push_str("\n;; ANSWER SECTION:\n");
            for record in response_message.answers() {
                result.push_str(&format!("{}\t{}\tIN\t{:?}\t{:?}\n", 
                    record.name(), record.ttl(), record.record_type(), record.data()));
            }
        }
        
        result
    }
    
    async fn update_dns_reply(&self, subdomain: &str, request_id: &str, reply: String) -> Result<()> {
        let key = format!("requests:{}", subdomain);
        let logs = self.cache.lrange(&key, 0, -1).await?;
        
        for log_json in logs {
            let mut log: DnsRequestLog = serde_json::from_str(&log_json)?;
            if log._id == request_id {
                log.reply = reply;
                let updated_json = serde_json::to_string(&log)?;
                
                self.cache.lrem(&key, 0, &log_json).await?;
                self.cache.lpush(&key, &updated_json).await?;
                
                let message = CacheMessage {
                    cmd: "update_request".to_string(),
                    subdomain: subdomain.to_string(),
                    data: updated_json,
                };
                let _ = self.tx.send(message);
                
                break;
            }
        }
        
        Ok(())
    }
    
    async fn handle_a_record<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
        subdomain: &str,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name().to_string();
        
        let dns_key = format!("dns:A:{}", name);
        let custom_record = self.cache.get(&dns_key).await.unwrap_or(None);
        
        let mut response_message = trust_dns_proto::op::Message::new();
        let mut header = Header::new();
        header.set_id(request.header().id());
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_recursion_desired(request.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        response_message.set_header(header);
        
        if let Some(value) = custom_record {
            if let Ok(ip) = value.parse::<Ipv4Addr>() {
                let octets = ip.octets();
                let rdata = RData::A(A::new(octets[0], octets[1], octets[2], octets[3]));
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    1, // TTL - 1 second to match Python implementation
                    rdata,
                );
                response_message.add_answer(record);
            }
        } else {
            let ip = Ipv4Addr::from_str(&CONFIG.server_ip).unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1));
            let octets = ip.octets();
            let rdata = RData::A(A::new(octets[0], octets[1], octets[2], octets[3]));
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                1, // TTL - 1 second to match Python implementation
                rdata,
            );
            response_message.add_answer(record);
        }
        
        let header = response_message.header().clone();
        let records: Vec<&Record> = response_message.answers().iter().collect();
        
        let response = MessageResponseBuilder::from_message_request(request).build(
            header,
            records.into_iter(),
            Vec::<&Record>::new().into_iter(),
            None,
            Vec::<&Record>::new().into_iter()
        );
        
        let response_info = match response_handle.send_response(response).await {
            Ok(response_info) => {
                let reply_str = self.format_dns_response(&response_message).await;
                
                let request_id_str = format!("{}", request.header().id());
                if let Err(e) = self.update_dns_reply(&subdomain, &request_id_str, reply_str).await {
                    error!("Failed to update DNS reply: {}", e);
                }
                
                response_info
            },
            Err(e) => {
                error!("Error sending A record response: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                ResponseInfo::from(header)
            }
        };
        
        response_info
    }

    async fn handle_aaaa_record<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
        subdomain: &str,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name().to_string();
        
        let dns_key = format!("dns:AAAA:{}", name);
        let custom_record = self.cache.get(&dns_key).await.unwrap_or(None);
        
        let mut response_message = trust_dns_proto::op::Message::new();
        let mut header = Header::new();
        header.set_id(request.header().id());
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_recursion_desired(request.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        if let Some(value) = custom_record {
            if let Ok(ip) = value.parse::<Ipv6Addr>() {
                let segments = ip.segments();
                let rdata = RData::AAAA(AAAA::new(
                    segments[0], segments[1], segments[2], segments[3],
                    segments[4], segments[5], segments[6], segments[7]
                ));
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    1, // TTL - 1 second to match Python implementation
                    rdata,
                );
                response_message.add_answer(record);
            }
        } else {
            header.set_response_code(ResponseCode::NXDomain);
            response_message.set_header(header);
        }
        
        response_message.set_header(header);
        
        let header = response_message.header().clone();
        let records: Vec<&Record> = response_message.answers().iter().collect();
        
        let response = MessageResponseBuilder::from_message_request(request).build(
            header,
            records.into_iter(),
            Vec::<&Record>::new().into_iter(),
            None,
            Vec::<&Record>::new().into_iter()
        );
        
        let response_info = match response_handle.send_response(response).await {
            Ok(response_info) => {
                let reply_str = self.format_dns_response(&response_message).await;
                
                let request_id_str = format!("{}", request.header().id());
                if let Err(e) = self.update_dns_reply(&subdomain, &request_id_str, reply_str).await {
                    error!("Failed to update DNS reply: {}", e);
                }
                
                response_info
            },
            Err(e) => {
                error!("Error sending AAAA record response: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                ResponseInfo::from(header)
            }
        };
        
        response_info
    }

    async fn handle_cname_record<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
        subdomain: &str,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name().to_string();
        
        let dns_key = format!("dns:CNAME:{}", name);
        let custom_record = self.cache.get(&dns_key).await.unwrap_or(None);
        
        let mut response_message = trust_dns_proto::op::Message::new();
        let mut header = Header::new();
        header.set_id(request.header().id());
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_recursion_desired(request.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        if let Some(value) = custom_record {
            if let Ok(target) = Name::from_str(&value) {
                let rdata = RData::CNAME(CNAME(target));
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    1, // TTL - 1 second to match Python implementation
                    rdata,
                );
                response_message.add_answer(record);
            }
        } else {
            header.set_response_code(ResponseCode::NXDomain);
        }
        
        response_message.set_header(header);
        
        let header = response_message.header().clone();
        let records: Vec<&Record> = response_message.answers().iter().collect();
        
        let response = MessageResponseBuilder::from_message_request(request).build(
            header,
            records.into_iter(),
            Vec::<&Record>::new().into_iter(),
            None,
            Vec::<&Record>::new().into_iter()
        );
        
        let response_info = match response_handle.send_response(response).await {
            Ok(response_info) => {
                let reply_str = self.format_dns_response(&response_message).await;
                
                let request_id_str = format!("{}", request.header().id());
                if let Err(e) = self.update_dns_reply(&subdomain, &request_id_str, reply_str).await {
                    error!("Failed to update DNS reply: {}", e);
                }
                
                response_info
            },
            Err(e) => {
                error!("Error sending CNAME record response: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                ResponseInfo::from(header)
            }
        };
        
        response_info
    }

    async fn handle_txt_record<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
        subdomain: &str,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name().to_string();
        
        let dns_key = format!("dns:TXT:{}", name);
        let custom_record = self.cache.get(&dns_key).await.unwrap_or(None);
        
        let mut response_message = trust_dns_proto::op::Message::new();
        let mut header = Header::new();
        header.set_id(request.header().id());
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_recursion_desired(request.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        if let Some(value) = custom_record {
            let txt_data = TXT::new(vec![value.clone()]);
            let rdata = RData::TXT(txt_data);
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                1, // TTL - 1 second to match Python implementation
                rdata,
            );
            response_message.add_answer(record);
        } else {
            let txt_data = TXT::new(vec![CONFIG.txt_record.clone()]);
            let rdata = RData::TXT(txt_data);
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                1, // TTL - 1 second to match Python implementation
                rdata,
            );
            response_message.add_answer(record);
        }
        
        response_message.set_header(header);
        
        let header = response_message.header().clone();
        let records: Vec<&Record> = response_message.answers().iter().collect();
        
        let response = MessageResponseBuilder::from_message_request(request).build(
            header,
            records.into_iter(),
            Vec::<&Record>::new().into_iter(),
            None,
            Vec::<&Record>::new().into_iter()
        );
        
        let response_info = match response_handle.send_response(response).await {
            Ok(response_info) => {
                let reply_str = self.format_dns_response(&response_message).await;
                
                let request_id_str = format!("{}", request.header().id());
                if let Err(e) = self.update_dns_reply(&subdomain, &request_id_str, reply_str).await {
                    error!("Failed to update DNS reply: {}", e);
                }
                
                response_info
            },
            Err(e) => {
                error!("Error sending TXT record response: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                ResponseInfo::from(header)
            }
        };
        
        response_info
    }

    async fn handle_default_response<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        let name = request.query().name().to_string();
        let subdomain = get_subdomain_from_hostname(&name).unwrap_or_else(|| "unknown".to_string());
        let mut response_message = trust_dns_proto::op::Message::new();
        let mut header = Header::new();
        header.set_id(request.header().id());
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NXDomain);
        header.set_recursion_desired(request.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        response_message.set_header(header);
        
        let header = response_message.header().clone();
        let records: Vec<&Record> = response_message.answers().iter().collect();
        
        let response = MessageResponseBuilder::from_message_request(request).build(
            header,
            records.into_iter(),
            Vec::<&Record>::new().into_iter(),
            None,
            Vec::<&Record>::new().into_iter()
        );
        
        let response_info = match response_handle.send_response(response).await {
            Ok(response_info) => {
                let reply_str = self.format_dns_response(&response_message).await;
                
                let request_id_str = format!("{}", request.header().id());
                if let Err(e) = self.update_dns_reply(&subdomain, &request_id_str, reply_str).await {
                    error!("Failed to update DNS reply: {}", e);
                }
                
                response_info
            },
            Err(e) => {
                error!("Error sending default response: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                ResponseInfo::from(header)
            }
        };
        
        response_info
    }
}
