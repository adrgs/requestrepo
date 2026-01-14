use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::cache::Cache;
use crate::ip2country::lookup_country;
use crate::models::{CacheMessage, TcpRequestLog};
use crate::utils::config::CONFIG;
use crate::utils::{generate_request_id, get_current_timestamp};

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    port_allocations: Arc<RwLock<HashMap<String, u16>>>,
    allocated_ports: Arc<RwLock<HashMap<u16, String>>>,
}

impl Server {
    pub fn new(cache: Arc<Cache>, tx: Arc<broadcast::Sender<CacheMessage>>) -> Self {
        Self {
            cache,
            tx,
            port_allocations: Arc::new(RwLock::new(HashMap::new())),
            allocated_ports: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting TCP port allocation service");

        self.start_port_allocation_service().await?;

        Ok(())
    }

    async fn start_port_allocation_service(&self) -> Result<()> {
        let mut rx = self.tx.subscribe();

        loop {
            match rx.recv().await {
                Ok(message) => {
                    if message.cmd == "allocate_tcp_port" {
                        let subdomain = message.subdomain;

                        if let Ok(port) = self.allocate_port(&subdomain) {
                            self.start_port_listener(port, subdomain.clone()).await?;

                            let response = CacheMessage {
                                cmd: "tcp_port_allocated".to_string(),
                                subdomain: subdomain.clone(),
                                data: port.to_string(),
                            };

                            let _ = self.tx.send(response);
                        }
                    } else if message.cmd == "release_tcp_port" {
                        let subdomain = message.subdomain;

                        self.release_port(&subdomain);
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                }
            }
        }
    }

    fn allocate_port(&self, subdomain: &str) -> Result<u16> {
        let mut allocations = self
            .port_allocations
            .write()
            .map_err(|_| anyhow!("Failed to acquire write lock"))?;
        let mut allocated = self
            .allocated_ports
            .write()
            .map_err(|_| anyhow!("Failed to acquire write lock"))?;

        if let Some(port) = allocations.get(subdomain) {
            return Ok(*port);
        }

        let start = CONFIG.tcp_port_range_start;
        let end = CONFIG.tcp_port_range_end;

        for port in start..=end {
            if let std::collections::hash_map::Entry::Vacant(e) = allocated.entry(port) {
                allocations.insert(subdomain.to_string(), port);
                e.insert(subdomain.to_string());

                info!("Allocated port {} for subdomain {}", port, subdomain);

                return Ok(port);
            }
        }

        Err(anyhow!("No available ports"))
    }

    fn release_port(&self, subdomain: &str) {
        if let Ok(mut allocations) = self.port_allocations.write() {
            if let Some(port) = allocations.remove(subdomain) {
                if let Ok(mut allocated) = self.allocated_ports.write() {
                    allocated.remove(&port);
                    info!("Released port {} for subdomain {}", port, subdomain);
                }
            }
        }
    }

    async fn start_port_listener(&self, port: u16, subdomain: String) -> Result<()> {
        let listener = match TcpListener::bind(format!("0.0.0.0:{port}")).await {
            Ok(listener) => listener,
            Err(e) => {
                error!("Failed to bind to port {}: {}", port, e);

                self.release_port(&subdomain);

                return Err(anyhow!("Failed to bind to port {}: {}", port, e));
            }
        };

        info!("Listening on port {} for subdomain {}", port, subdomain);

        let cache = self.cache.clone();
        let tx = self.tx.clone();
        let subdomain_clone = subdomain.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let cache = cache.clone();
                        let tx = tx.clone();
                        let subdomain = subdomain_clone.clone();

                        tokio::spawn(async move {
                            if let Err(e) =
                                handle_tcp_connection(socket, addr, port, &subdomain, cache, tx)
                                    .await
                            {
                                error!("Error handling TCP connection: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting TCP connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}

async fn handle_tcp_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    port: u16,
    subdomain: &str,
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
) -> Result<()> {
    let mut buffer = [0u8; 8192];

    let client_ip = addr.ip().to_string();

    let n = socket.read(&mut buffer).await?;

    if n > 0 {
        let request_id = generate_request_id();

        let country = lookup_country(&client_ip);

        let request_log = TcpRequestLog {
            _id: request_id.clone(),
            r#type: "tcp".to_string(),
            raw: BASE64.encode(&buffer[..n]),
            uid: subdomain.to_string(),
            port,
            date: get_current_timestamp(),
            ip: Some(client_ip),
            country,
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

        socket.write_all(&buffer[..n]).await?;
    }

    Ok(())
}
