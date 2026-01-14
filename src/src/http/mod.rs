mod routes;
mod routes_v2;
mod websocket;

use anyhow::{anyhow, Result};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit},
    http::Method,
    routing::{get, post},
    Router,
};
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower::{Layer, Service};
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

use crate::cache::Cache;
use crate::certs::TlsManager;
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

        let app = create_router(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.http_port));
        let listener = TcpListener::bind(addr).await?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| anyhow!("HTTP server error: {}", e))?;

        Ok(())
    }
}

/// Create the shared router with all routes
fn create_router(state: AppState) -> Router {
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

    // API routes - these need CORS for browser requests
    let api_routes = Router::new()
        .route("/health", get(routes::health))
        .route("/api/v2/sessions", post(routes_v2::create_session))
        .route(
            "/api/v2/dns",
            get(routes_v2::get_dns).put(routes_v2::update_dns),
        )
        .route(
            "/api/v2/files",
            get(routes_v2::get_files).put(routes_v2::update_files),
        )
        .route("/api/v2/files/*path", get(routes_v2::get_file))
        .route(
            "/api/v2/requests",
            get(routes_v2::list_requests).delete(routes_v2::delete_all_requests),
        )
        .route(
            "/api/v2/requests/shared/:token",
            get(routes_v2::get_shared_request),
        )
        .route("/api/v2/requests/:id/share", post(routes_v2::share_request))
        .route(
            "/api/v2/requests/:id",
            get(routes_v2::get_request).delete(routes_v2::delete_request),
        )
        .route("/api/v2/ws", get(websocket::websocket_handler_v2))
        .layer(cors);

    // Main router: API routes with CORS, catch_all WITHOUT CORS
    // This gives users full control over response headers for their files
    Router::new()
        .merge(api_routes)
        .fallback(routes::catch_all)
        .layer(DefaultBodyLimit::max(CONFIG.max_request_body_bytes))
        .with_state(state)
}

/// HTTPS server that runs alongside the HTTP server
pub struct HttpsServer {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    tls: TlsManager,
}

impl HttpsServer {
    pub fn new(
        cache: Arc<Cache>,
        tx: Arc<broadcast::Sender<CacheMessage>>,
        tls: TlsManager,
    ) -> Self {
        Self { cache, tx, tls }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.https_port));
        info!("Starting HTTPS server on port {}", CONFIG.https_port);

        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, remote_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            // Get TLS acceptor - skip if no cert configured yet
            let acceptor = match self.tls.acceptor() {
                Some(a) => a,
                None => {
                    warn!("HTTPS connection rejected: no certificate configured");
                    continue;
                }
            };

            // Clone state for the spawned task
            let state = AppState {
                cache: self.cache.clone(),
                tx: self.tx.clone(),
            };

            tokio::spawn(async move {
                // Perform TLS handshake
                let tls_stream = match acceptor.accept(stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("TLS handshake failed from {}: {}", remote_addr, e);
                        return;
                    }
                };

                // Create router with ConnectInfo injected for this connection
                let router = create_router(state).layer(InjectConnectInfo(remote_addr));
                let service = TowerToHyperService::new(router);

                let io = TokioIo::new(tls_stream);

                // Use http1 builder with upgrades enabled for WebSocket support
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .with_upgrades()
                    .await
                {
                    // Don't log connection reset errors as they're common
                    if !e.to_string().contains("connection reset") {
                        error!("Error serving HTTPS connection: {}", e);
                    }
                }
            });
        }
    }
}

// Layer to inject ConnectInfo into requests (used for HTTPS where we manually accept connections)
#[derive(Clone, Copy)]
struct InjectConnectInfo(SocketAddr);

impl<S> Layer<S> for InjectConnectInfo {
    type Service = InjectConnectInfoService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InjectConnectInfoService {
            inner,
            addr: self.0,
        }
    }
}

#[derive(Clone)]
struct InjectConnectInfoService<S> {
    inner: S,
    addr: SocketAddr,
}

impl<S, ReqBody> Service<axum::http::Request<ReqBody>> for InjectConnectInfoService<S>
where
    S: Service<axum::http::Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: axum::http::Request<ReqBody>) -> Self::Future {
        req.extensions_mut().insert(ConnectInfo(self.addr));
        self.inner.call(req)
    }
}
