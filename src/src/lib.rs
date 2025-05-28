
mod cache;
mod dns;
mod http;
mod ip2country;
mod models;
mod smtp;
mod tcp;
mod utils;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

pub async fn run() -> Result<()> {
    info!("Starting RequestRepo backend");

    let cache = Arc::new(cache::Cache::new());
    
    let (tx, _) = broadcast::channel(1024);
    let tx = Arc::new(tx);

    let dns_server = dns::Server::new(cache.clone(), tx.clone());
    let dns_handle = tokio::spawn(async move {
        if let Err(e) = dns_server.run().await {
            tracing::error!("DNS server error: {}", e);
        }
    });

    let http_server = http::Server::new(cache.clone(), tx.clone());
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_server.run().await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    let smtp_server = smtp::Server::new(cache.clone(), tx.clone());
    let smtp_handle = tokio::spawn(async move {
        if let Err(e) = smtp_server.run().await {
            tracing::error!("SMTP server error: {}", e);
        }
    });

    let tcp_server = tcp::Server::new(cache.clone(), tx.clone());
    let tcp_handle = tokio::spawn(async move {
        if let Err(e) = tcp_server.run().await {
            tracing::error!("TCP server error: {}", e);
        }
    });

    let _ = tokio::join!(dns_handle, http_handle, smtp_handle, tcp_handle);

    Ok(())
}
