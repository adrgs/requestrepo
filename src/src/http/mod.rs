
mod routes;
mod websocket;

use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info};

use crate::cache::Cache;
use crate::models::CacheMessage;
use crate::utils::config::CONFIG;

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
}

#[derive(Clone)]
pub struct AppState {
    pub cache: Arc<Cache>,
    pub tx: Arc<broadcast::Sender<CacheMessage>>,
}

impl Server {
    pub fn new(cache: Arc<Cache>, tx: Arc<broadcast::Sender<CacheMessage>>) -> Self {
        Self { cache, tx }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting HTTP server on port {}", CONFIG.http_port);

        let state = AppState {
            cache: self.cache.clone(),
            tx: self.tx.clone(),
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/update_dns", post(routes::update_dns))
            .route("/api/get_dns", get(routes::get_dns))
            .route("/api/get_file", get(routes::get_file))
            .route("/api/get_request", get(routes::get_request))
            .route("/api/delete_request", post(routes::delete_request))
            .route("/api/delete_all", post(routes::delete_all))
            .route("/api/update_file", post(routes::update_file))
            .route("/api/get_token", post(routes::get_token))
            .route("/api/files", get(routes::get_files).post(routes::update_files))
            .route("/api/ws", get(websocket::websocket_handler))
            .route("/api/ws2", get(websocket::websocket_handler_v2))
            .fallback(routes::catch_all)
            .with_state(state)
            .layer(cors);

        let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.http_port));

        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .map_err(|e| anyhow!("HTTP server error: {}", e))?;

        Ok(())
    }
}
