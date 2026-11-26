//! A real server on a loopback port, and a bare HTTP/1.1 and WebSocket client to talk to
//! it, so that middleware, routing and the API are exercised exactly as deployed.

#![cfg(test)]

use crate::abuse::{AbuseList, Settings};
use crate::cache::Cache;
use crate::http::{create_router, AppState, StaticFiles};
use crate::utils::config::CONFIG;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

/// ADMIN_TOKEN of a private instance.
pub const ADMIN: &str = "instance-admin-token-for-tests";
pub const JSON: &[(&str, &str)] = &[("Content-Type", "application/json")];

pub struct TestServer {
    pub addr: SocketAddr,
    pub cache: Arc<Cache>,
    pub abuse: Arc<AbuseList>,
    pub data_dir: TempDir,
}

/// Settings of a private instance, which has an ADMIN_TOKEN.
pub fn private_settings() -> Settings {
    Settings {
        admin_token: Some(ADMIN.to_string()),
        ..Settings::default()
    }
}

impl TestServer {
    pub async fn start(settings: Settings) -> Self {
        let data_dir = TempDir::new().unwrap();
        let abuse = Arc::new(AbuseList::load(data_dir.path(), settings).await.unwrap());
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let state = AppState {
            cache: cache.clone(),
            tx: Arc::new(tx),
            static_files: Arc::new(StaticFiles::load("./public").await),
            http_challenge_handler: None,
            abuse: abuse.clone(),
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                create_router(state).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });

        Self {
            addr,
            cache,
            abuse,
            data_dir,
        }
    }

    /// The main domain, where the dashboard and the API live.
    pub fn main_host(&self) -> String {
        CONFIG.server_domain.clone()
    }

    /// The host a subdomain's own traffic arrives with.
    pub fn subdomain_host(&self, subdomain: &str) -> String {
        format!("{subdomain}.{}", CONFIG.server_domain)
    }

    pub async fn request(
        &self,
        method: &str,
        host: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Reply {
        http(self.addr, method, host, target, headers, body).await
    }

    /// A request to the main domain.
    pub async fn main(&self, method: &str, target: &str, headers: &[(&str, &str)]) -> Reply {
        self.request(method, &self.main_host(), target, headers, "")
            .await
    }

    /// A request to the API on the main domain with `Authorization: Bearer <token>`. A
    /// non-empty `body` is sent as JSON.
    pub async fn api(&self, token: &str, method: &str, target: &str, body: &str) -> Reply {
        let authorization = format!("Bearer {token}");
        let mut headers = vec![("Authorization", authorization.as_str())];
        if !body.is_empty() {
            headers.push(("Content-Type", "application/json"));
        }
        self.request(method, &self.main_host(), target, &headers, body)
            .await
    }
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("response is not JSON ({e}): {}", self.body))
    }

    pub fn code(&self) -> String {
        self.json()["code"].as_str().unwrap().to_string()
    }
}

/// Sends one HTTP/1.1 request with `Connection: close` and reads the whole response.
pub async fn http(
    addr: SocketAddr,
    method: &str,
    host: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut request =
        format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if !body.is_empty() {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    // The server may close before a body it has already declined arrives
    let _ = stream.write_all(body.as_bytes()).await;

    let mut raw = Vec::new();
    // A server that answers without reading the body closes the connection, which a client
    // still sending one sees as a reset after the response. Only a reset before a whole
    // response is a failure; say which request it belonged to.
    if let Err(e) = stream.read_to_end(&mut raw).await {
        if raw.windows(4).any(|window| window == b"\r\n\r\n") {
            return parse_response(&raw);
        }
        panic!(
            "reading the response to {method} {target} failed after {} bytes: {e}",
            raw.len()
        );
    }
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Reply {
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| {
            panic!(
                "response has no header terminator: {}",
                String::from_utf8_lossy(raw)
            )
        });
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let body = String::from_utf8_lossy(&raw[split + 4..]).to_string();

    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap();
    let status: u16 = status_line.split(' ').nth(1).unwrap().parse().unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    Reply {
        status,
        headers,
        body,
    }
}

/// Opens a WebSocket and returns the upgraded stream.
pub async fn websocket(addr: SocketAddr, host: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "GET /api/v2/ws HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();

    // Read the response head past the upgrade; the socket then carries frames
    let mut head = Vec::new();
    let mut byte = [0u8];
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await.unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    stream
}

/// Sends one masked text frame, as a browser would.
pub async fn ws_send(stream: &mut TcpStream, text: &str) {
    assert!(text.len() <= u16::MAX as usize, "test frames are short");
    const MASK: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
    let mut frame = vec![0x81];
    match text.len() {
        n if n < 126 => frame.push(0x80 | n as u8),
        n => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
    }
    frame.extend_from_slice(&MASK);
    frame.extend(
        text.bytes()
            .zip(MASK.iter().cycle())
            .map(|(byte, mask)| byte ^ mask),
    );
    stream.write_all(&frame).await.unwrap();
}

/// Reads one text frame and returns its payload.
pub async fn ws_recv(stream: &mut TcpStream) -> Value {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.unwrap();
    assert_eq!(head[0] & 0x0f, 0x1, "expected a text frame");
    assert_eq!(head[1] & 0x80, 0, "server frames are not masked");
    let length = match head[1] & 0x7f {
        n if n < 126 => n as usize,
        126 => {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended).await.unwrap();
            u16::from_be_bytes(extended) as usize
        }
        _ => panic!("test frames are short"),
    };
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await.unwrap();
    serde_json::from_slice(&payload).expect("frame is JSON")
}
