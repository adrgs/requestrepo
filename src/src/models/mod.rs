
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CasePreservingHeaders(pub HashMap<String, String>);

impl Serialize for CasePreservingHeaders {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for CasePreservingHeaders {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = HashMap::deserialize(deserializer)?;
        Ok(CasePreservingHeaders(map))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestLog {
    pub _id: String,
    pub r#type: String,
    pub raw: String,
    pub uid: String,
    pub method: String,
    pub path: String,
    pub headers: CasePreservingHeaders,
    pub date: i64,
    pub ip: Option<String>,
    pub country: Option<String>,
    pub port: i32,
    pub protocol: String,
    pub fragment: String,
    pub query: String,
    pub url: String,
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
    #[serde(rename = "type")]
    #[serde(deserialize_with = "deserialize_dns_record_type")]
    pub r#type: String,
    pub value: String,
}

fn deserialize_dns_record_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};
    use std::fmt;
    
    struct DnsRecordTypeVisitor;
    
    impl<'de> Visitor<'de> for DnsRecordTypeVisitor {
        type Value = String;
        
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or an integer representing DNS record type")
        }
        
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(value.to_string())
        }
        
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(value)
        }
        
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let dns_record_types = ["A", "AAAA", "CNAME", "TXT"];
            
            if value as usize >= dns_record_types.len() {
                return Err(Error::custom(format!("Invalid DNS record type index: {}", value)));
            }
            
            Ok(dns_record_types[value as usize].to_string())
        }
    }
    
    deserializer.deserialize_any(DnsRecordTypeVisitor)
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
