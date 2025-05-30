use std::net::SocketAddr;
use axum::{Router, extract::connect_info::IntoMakeServiceWithConnectInfo};
use tokio::net::TcpListener;
use tracing::info;
use anyhow::Result;

pub async fn run_custom_server(
    app: IntoMakeServiceWithConnectInfo<Router, SocketAddr>, 
    addr: SocketAddr
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);
    info!("HTTP server configured with header case preservation");
    
    std::env::set_var("HYPER_HTTP1_PRESERVE_HEADER_CASE", "1");
    std::env::set_var("HYPER_HTTP1_PRESERVE_HEADER_CASE_PERCENT_ENCODE", "1");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
