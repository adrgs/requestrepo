
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestLog {
    pub _id: String,
    pub r#type: String,
    pub raw: String,
    pub uid: String,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub date: i64,
    pub ip: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRequestLog {
    pub _id: String,
    pub r#type: String,
    pub raw: String,
    pub uid: String,
    pub query_type: String,
    pub domain: String,
    pub date: i64,
    pub ip: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpRequestLog {
    pub _id: String,
    pub r#type: String,
    pub raw: String,
    pub uid: String,
    pub command: String,
    pub data: Option<String>,
    pub date: i64,
    pub ip: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpRequestLog {
    pub _id: String,
    pub r#type: String,
    pub raw: String,
    pub uid: String,
    pub port: u16,
    pub date: i64,
    pub ip: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub domain: String,
    pub r#type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecords {
    pub records: Vec<DnsRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub header: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub raw: String,
    pub headers: Vec<Header>,
    pub status_code: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTree {
    #[serde(flatten)]
    pub files: HashMap<String, Response>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub iat: i64,
    pub exp: i64,
    pub subdomain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMessage {
    pub cmd: String,
    pub subdomain: String,
    pub data: String,
}
