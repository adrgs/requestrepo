
use anyhow::{anyhow, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use trust_dns_proto::op::{Header, MessageType, OpCode, ResponseCode};
use trust_dns_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use trust_dns_server::authority::{Authority, Catalog, ZoneType};
use trust_dns_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
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
        let socket = Arc::new(socket);

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

struct DnsRequestHandler {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
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

        let subdomain = match get_subdomain_from_hostname(&name) {
            Some(subdomain) => subdomain,
            None => {
                return self.handle_default_response(request, response_handle).await;
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
        
        let request_log = DnsRequestLog {
            _id: request_id.clone(),
            r#type: "dns".to_string(),
            raw: base64::encode(format!("{:?}", request)),
            uid: subdomain.to_string(),
            query_type: format!("{:?}", query_type),
            domain: name,
            date: get_current_timestamp(),
            ip: Some(source_ip),
            country,
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
        
        let mut header = Header::new();
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_id(query.id());
        header.set_recursion_desired(query.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        let mut records = Vec::new();
        
        if let Some(value) = custom_record {
            if let Ok(ip) = value.parse::<Ipv4Addr>() {
                let rdata = RData::A(ip);
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    300, // TTL
                    rdata,
                );
                records.push(record);
            }
        } else {
            let ip = Ipv4Addr::from_str(&CONFIG.server_ip).unwrap_or_else(|_| Ipv4Addr::new(127, 0, 0, 1));
            let rdata = RData::A(ip);
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                300, // TTL
                rdata,
            );
            records.push(record);
        }
        
        let result = response_handle.send_response(header, records.iter()).await;
        
        match result {
            Ok(response_info) => response_info,
            Err(e) => {
                error!("Error sending A record response: {}", e);
                ResponseInfo::new()
            }
        }
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
        
        let mut header = Header::new();
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_id(query.id());
        header.set_recursion_desired(query.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        let mut records = Vec::new();
        
        if let Some(value) = custom_record {
            if let Ok(ip) = value.parse::<Ipv6Addr>() {
                let rdata = RData::AAAA(ip);
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    300, // TTL
                    rdata,
                );
                records.push(record);
            }
        } else {
            header.set_response_code(ResponseCode::NXDomain);
        }
        
        let result = response_handle.send_response(header, records.iter()).await;
        
        match result {
            Ok(response_info) => response_info,
            Err(e) => {
                error!("Error sending AAAA record response: {}", e);
                ResponseInfo::new()
            }
        }
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
        
        let mut header = Header::new();
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_id(query.id());
        header.set_recursion_desired(query.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        let mut records = Vec::new();
        
        if let Some(value) = custom_record {
            if let Ok(target) = Name::from_str(&value) {
                let rdata = RData::CNAME(target);
                let record = Record::from_rdata(
                    Name::from_str(&name).unwrap(),
                    300, // TTL
                    rdata,
                );
                records.push(record);
            }
        } else {
            header.set_response_code(ResponseCode::NXDomain);
        }
        
        let result = response_handle.send_response(header, records.iter()).await;
        
        match result {
            Ok(response_info) => response_info,
            Err(e) => {
                error!("Error sending CNAME record response: {}", e);
                ResponseInfo::new()
            }
        }
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
        
        let mut header = Header::new();
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NoError);
        header.set_id(query.id());
        header.set_recursion_desired(query.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        let mut records = Vec::new();
        
        if let Some(value) = custom_record {
            let rdata = RData::TXT(vec![value.into_bytes()]);
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                300, // TTL
                rdata,
            );
            records.push(record);
        } else {
            let rdata = RData::TXT(vec![CONFIG.txt_record.clone().into_bytes()]);
            let record = Record::from_rdata(
                Name::from_str(&name).unwrap(),
                300, // TTL
                rdata,
            );
            records.push(record);
        }
        
        let result = response_handle.send_response(header, records.iter()).await;
        
        match result {
            Ok(response_info) => response_info,
            Err(e) => {
                error!("Error sending TXT record response: {}", e);
                ResponseInfo::new()
            }
        }
    }

    async fn handle_default_response<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        let query = request.query();
        
        let mut header = Header::new();
        header.set_message_type(MessageType::Response);
        header.set_op_code(OpCode::Query);
        header.set_response_code(ResponseCode::NXDomain);
        header.set_id(query.id());
        header.set_recursion_desired(query.header().recursion_desired());
        header.set_recursion_available(true);
        header.set_authoritative(true);
        
        let result = response_handle.send_response(header, &[]).await;
        
        match result {
            Ok(response_info) => response_info,
            Err(e) => {
                error!("Error sending default response: {}", e);
                ResponseInfo::new()
            }
        }
    }
}
