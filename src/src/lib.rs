mod cache;
pub(crate) mod certs;
mod dns;
mod http;
mod ip2country;
mod models;
mod smtp;
mod tcp;
mod utils;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use utils::config::CONFIG;

pub async fn run() -> Result<()> {
    info!("Starting RequestRepo backend");

    // Initialize IP2Country database
    if let Err(e) = ip2country::init() {
        warn!("Failed to initialize IP2Country database: {}", e);
    }

    let cache = Arc::new(cache::Cache::new());

    // Start the cache cleanup task
    cache::Cache::start_cleanup_task(cache.clone());

    let (tx, _) = broadcast::channel(1024);
    let tx = Arc::new(tx);

    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Load static frontend files into memory (shared between HTTP and HTTPS)
    let static_files = http::load_static_files().await;

    let dns_server = dns::Server::new(cache.clone(), tx.clone());
    let dns_handle = tokio::spawn(async move {
        if let Err(e) = dns_server.run().await {
            error!("DNS server error: {}", e);
        }
    });

    let http_server = http::Server::new(cache.clone(), tx.clone(), static_files.clone());
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_server.run().await {
            error!("HTTP server error: {}", e);
        }
    });

    let smtp_server = smtp::Server::new(cache.clone(), tx.clone());
    let smtp_handle = tokio::spawn(async move {
        if let Err(e) = smtp_server.run().await {
            error!("SMTP server error: {}", e);
        }
    });

    let tcp_server = tcp::Server::new(cache.clone(), tx.clone());
    let tcp_handle = tokio::spawn(async move {
        if let Err(e) = tcp_server.run().await {
            error!("TCP server error: {}", e);
        }
    });

    // Initialize HTTPS server if TLS is enabled
    let https_handle = if CONFIG.tls_enabled {
        info!("TLS is enabled, initializing certificate manager");

        match certs::CertManager::new(cache.clone()).await {
            Ok(cert_manager) => {
                let cert_manager = Arc::new(cert_manager);

                // Start background renewal task
                certs::CertManager::start_renewal_task(cert_manager.clone());

                // Start HTTPS server
                let https_server = http::HttpsServer::new(
                    cache.clone(),
                    tx.clone(),
                    cert_manager.tls_manager(),
                    static_files.clone(),
                );

                Some(tokio::spawn(async move {
                    if let Err(e) = https_server.run().await {
                        error!("HTTPS server error: {}", e);
                    }
                }))
            }
            Err(e) => {
                error!("Failed to initialize certificate manager: {}", e);
                warn!("HTTPS server will not be started");
                None
            }
        }
    } else {
        info!("TLS is disabled, skipping HTTPS server");
        None
    };

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, initiating graceful shutdown...");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("Failed to register SIGTERM handler");
                sigterm.recv().await
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await
            }
        } => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        }
        _ = async {
            let _ = tokio::join!(
                dns_handle,
                http_handle,
                smtp_handle,
                tcp_handle,
            );
            // Also wait for HTTPS if running
            if let Some(handle) = https_handle {
                let _ = handle.await;
            }
        } => {
            // All servers exited on their own
        }
    }

    // Send shutdown signal to all tasks
    let _ = shutdown_tx.send(());

    info!("Shutdown complete");

    Ok(())
}
