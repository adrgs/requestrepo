use anyhow::{anyhow, Result};
use axum::Router;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};
use tokio_rustls::TlsAcceptor;
use tower::Service;
use tracing::{error, info};

use crate::utils::certificate::CertificateManager;
use crate::utils::config::CONFIG;

pub async fn run_https_server(app: Router) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.https_port));
    
    info!("Starting HTTPS server on port {}", CONFIG.https_port);
    
    let cert_manager = CertificateManager::new(&CONFIG.server_domain);
    
    let (cert_chain, private_key) = cert_manager.get_or_renew_certificate().await?;
    
    let tls_config = configure_tls(cert_chain.as_str(), private_key.as_str())?;
    let tls_acceptor = TlsAcceptor::from(tls_config);
    
    let listener = TcpListener::bind(&addr).await?;
    
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                continue;
            }
        };
        
        let acceptor = tls_acceptor.clone();
        let app = app.clone();
        
        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(stream).await {
                Ok(tls_stream) => tls_stream,
                Err(e) => {
                    error!("Failed to accept TLS connection: {}", e);
                    return;
                }
            };
            
            let _peer_addr = match tls_stream.get_ref().0.peer_addr() {
                Ok(addr) => addr,
                Err(_) => SocketAddr::from(([127, 0, 0, 1], 0)),
            };
            
            let io = TokioIo::new(tls_stream);
            
            let service = service_fn(move |req| {
                let app = app.clone();
                let mut app_svc = app.into_service();
                async move { app_svc.call(req).await }
            });
            
            if let Err(e) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(false)
                .serve_connection(io, service)
                .await
            {
                error!("Failed to serve connection: {}", e);
            }
        });
    }
}

fn configure_tls(cert_chain: &str, private_key: &str) -> Result<Arc<ServerConfig>> {
    let mut cert_reader = BufReader::new(cert_chain.as_bytes());
    let certs = rustls_pemfile::certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect::<Vec<_>>();
    
    if certs.is_empty() {
        return Err(anyhow!("No certificates found"));
    }
    
    let mut key_reader = BufReader::new(private_key.as_bytes());
    let key = rustls_pemfile::pkcs8_private_keys(&mut key_reader)?
        .into_iter()
        .map(PrivateKey)
        .next()
        .ok_or_else(|| anyhow!("No private key found"))?;
    
    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| anyhow!("TLS configuration error: {}", e))?;
    
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    
    Ok(Arc::new(config))
}

pub fn load_certs(path: &str) -> Result<Vec<Certificate>> {
    let cert_file = File::open(path)?;
    let mut reader = BufReader::new(cert_file);
    
    let certs = rustls_pemfile::certs(&mut reader)?
        .into_iter()
        .map(Certificate)
        .collect::<Vec<_>>();
    
    if certs.is_empty() {
        return Err(anyhow!("No certificates found in {}", path));
    }
    
    Ok(certs)
}

pub fn load_private_key(path: &str) -> Result<PrivateKey> {
    let key_file = File::open(path)?;
    let mut reader = BufReader::new(key_file);
    
    let key = rustls_pemfile::pkcs8_private_keys(&mut reader)?
        .into_iter()
        .map(PrivateKey)
        .next()
        .ok_or_else(|| anyhow!("No private key found in {}", path))?;
    
    Ok(key)
}
