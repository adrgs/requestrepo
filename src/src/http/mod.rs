mod routes;
mod routes_v2;
mod static_files;
mod websocket;

pub use static_files::StaticFiles;

/// Marker extension to indicate a request came over TLS
/// This is inserted by the HTTPS server and cannot be spoofed by clients
#[derive(Clone, Copy, Debug)]
pub struct TlsConnectInfo;

use anyhow::{anyhow, Result};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, Path, Request, State},
    http::{header, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use sentry_tower::{NewSentryLayer, SentryHttpLayer};
use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Semaphore};
use tower::{Layer, Service};
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

use crate::cache::Cache;
use crate::certs::{HttpChallengeHandler, TlsManager};
use crate::models::CacheMessage;
use crate::utils::config::CONFIG;

const HTTPS_MAX_CONCURRENT_CONNECTIONS: usize = 1024;
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum time a connection may spend waiting for request headers, applied to the first
/// request and again between keep-alive requests. Without it an idle keep-alive or
/// slow-loris connection pins its concurrency permit indefinitely; enough of them drain
/// HTTPS_MAX_CONCURRENT_CONNECTIONS, after which every new connection is refused before the
/// TLS handshake. That presents as a total HTTPS outage (resets, "no peer certificate")
/// even though the process, HTTP, and DNS all stay healthy.
const HTTP_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Minimum spacing between "connection pool exhausted" log lines, so a sustained flood
/// cannot itself become a log flood while never letting the condition go silent.
const PERMIT_EXHAUSTION_LOG_INTERVAL: Duration = Duration::from_secs(5);

fn sanitized_sentry_path(path: &str) -> Cow<'_, str> {
    const SHARED_REQUEST_PREFIX: &str = "/api/v2/requests/shared/";
    if path.strip_prefix(SHARED_REQUEST_PREFIX).is_some() {
        Cow::Borrowed("/api/v2/requests/shared/<redacted>")
    } else {
        Cow::Borrowed(path)
    }
}

fn sanitized_sentry_request(request: &Request) -> sentry::protocol::Request {
    let path = sanitized_sentry_path(request.uri().path());
    let url = request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .and_then(|host| format!("http://{host}{path}").parse().ok());

    sentry::protocol::Request {
        method: Some(request.method().to_string()),
        url,
        // Deliberately omit query strings and headers. Both can contain credentials,
        // and route/method data is sufficient for grouping and diagnostics.
        ..Default::default()
    }
}

/// Replace the request context initially captured by sentry-tower. This runs inside
/// SentryHttpLayer, so it sanitizes both error events and performance transactions.
async fn sanitize_sentry_context(request: Request, next: Next) -> Response {
    let sentry_request = sanitized_sentry_request(&request);
    let transaction_path = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|matched| Cow::Owned(matched.as_str().to_string()))
        .unwrap_or_else(|| sanitized_sentry_path(request.uri().path()));
    let transaction_name = format!("{} {transaction_path}", request.method());

    sentry::configure_scope(|scope| {
        let event_request = sentry_request.clone();
        scope.add_event_processor(move |mut event| {
            event.request = Some(event_request.clone());
            Some(event)
        });

        if let Some(span) = scope.get_span() {
            span.set_name(&transaction_name);
            span.set_request(sentry_request);
        }
    });

    next.run(request).await
}

pub struct Server {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    static_files: Arc<StaticFiles>,
    http_challenge_handler: Option<HttpChallengeHandler>,
}

#[derive(Clone)]
pub struct AppState {
    pub cache: Arc<Cache>,
    pub tx: Arc<broadcast::Sender<CacheMessage>>,
    pub static_files: Arc<StaticFiles>,
    pub http_challenge_handler: Option<HttpChallengeHandler>,
}

/// Load static files into memory (call once at startup, share between servers)
pub async fn load_static_files() -> Arc<StaticFiles> {
    Arc::new(StaticFiles::load("./public").await)
}

impl Server {
    pub fn new(
        cache: Arc<Cache>,
        tx: Arc<broadcast::Sender<CacheMessage>>,
        static_files: Arc<StaticFiles>,
        http_challenge_handler: Option<HttpChallengeHandler>,
    ) -> Self {
        Self {
            cache,
            tx,
            static_files,
            http_challenge_handler,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting HTTP server on port {}", CONFIG.http_port);

        let state = AppState {
            cache: self.cache.clone(),
            tx: self.tx.clone(),
            static_files: self.static_files.clone(),
            http_challenge_handler: self.http_challenge_handler.clone(),
        };

        let app = create_router(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.http_port));
        let listener = TcpListener::bind(addr).await?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| anyhow!("HTTP server error: {e}"))?;

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

    // Main router: API routes with CORS, ACME challenge, catch_all WITHOUT CORS
    // This gives users full control over response headers for their files
    Router::new()
        .merge(api_routes)
        .route(
            "/.well-known/acme-challenge/:token",
            get(acme_challenge_handler),
        )
        .fallback(routes::catch_all)
        .layer(DefaultBodyLimit::max(CONFIG.max_request_body_bytes))
        .layer(middleware::from_fn(sanitize_sentry_context))
        .layer(SentryHttpLayer::new().enable_transaction())
        .layer(NewSentryLayer::new_from_top())
        .with_state(state)
}

/// Handler for ACME HTTP-01 challenges (IP certificate validation)
async fn acme_challenge_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    if let Some(ref handler) = state.http_challenge_handler {
        if let Some(key_auth) = handler.get_response(&token) {
            return (
                StatusCode::OK,
                [("content-type", "application/octet-stream")],
                key_auth,
            )
                .into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

/// HTTPS server that runs alongside the HTTP server
pub struct HttpsServer {
    cache: Arc<Cache>,
    tx: Arc<broadcast::Sender<CacheMessage>>,
    tls: TlsManager,
    static_files: Arc<StaticFiles>,
    http_challenge_handler: Option<HttpChallengeHandler>,
}

impl HttpsServer {
    pub fn new(
        cache: Arc<Cache>,
        tx: Arc<broadcast::Sender<CacheMessage>>,
        tls: TlsManager,
        static_files: Arc<StaticFiles>,
        http_challenge_handler: Option<HttpChallengeHandler>,
    ) -> Self {
        Self {
            cache,
            tx,
            tls,
            static_files,
            http_challenge_handler,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.https_port));
        info!("Starting HTTPS server on port {}", CONFIG.https_port);

        let listener = TcpListener::bind(addr).await?;
        let connection_limit = Arc::new(Semaphore::new(HTTPS_MAX_CONCURRENT_CONNECTIONS));

        // Rate-limited reporting of pool-exhaustion drops. Both are only touched from this
        // single accept loop, so plain locals suffice — no synchronization needed.
        let mut permit_exhausted_drops: u64 = 0;
        let mut last_exhaustion_log: Option<Instant> = None;

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

            // Refuse excess connections before allocating a TLS task. The permit is held
            // through the TLS handshake and the HTTP-serving phase, then released (see the
            // explicit drop after serve_connection). header_read_timeout caps idle
            // keep-alive time so a permit can't be pinned indefinitely.
            let connection_permit = match connection_limit.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // Pool exhausted — refuse this connection. Log at most once per
                    // PERMIT_EXHAUSTION_LOG_INTERVAL so the drop is never silent (an
                    // exhausted pool means HTTPS is effectively down) without letting a
                    // flood of refusals turn into a flood of log lines.
                    permit_exhausted_drops += 1;
                    let due = last_exhaustion_log
                        .map(|t| t.elapsed() >= PERMIT_EXHAUSTION_LOG_INTERVAL)
                        .unwrap_or(true);
                    if due {
                        warn!(
                            "HTTPS connection pool exhausted ({} slots): refused {} connection(s) since last report",
                            HTTPS_MAX_CONCURRENT_CONNECTIONS, permit_exhausted_drops
                        );
                        permit_exhausted_drops = 0;
                        last_exhaustion_log = Some(Instant::now());
                    }
                    continue;
                }
            };

            // Clone state for the spawned task
            let state = AppState {
                cache: self.cache.clone(),
                tx: self.tx.clone(),
                static_files: self.static_files.clone(),
                http_challenge_handler: self.http_challenge_handler.clone(),
            };

            tokio::spawn(async move {
                // `connection_permit` (moved in above) is held for the HTTP-serving phase
                // and released explicitly once serving/upgrade completes, below.

                // Perform TLS handshake
                let tls_stream = match tokio::time::timeout(
                    TLS_HANDSHAKE_TIMEOUT,
                    acceptor.accept(stream),
                )
                .await
                {
                    Ok(Ok(stream)) => stream,
                    Ok(Err(e)) => {
                        error!("TLS handshake failed from {}: {}", remote_addr, e);
                        return;
                    }
                    Err(_) => {
                        tracing::debug!("TLS handshake timed out from {}", remote_addr);
                        return;
                    }
                };

                // Create router with ConnectInfo injected for this connection
                let router = create_router(state).layer(InjectConnectInfo(remote_addr));
                let service = TowerToHyperService::new(router);

                let io = TokioIo::new(tls_stream);

                // http1 with upgrades enabled for WebSocket support. header_read_timeout
                // bounds how long an idle or slow-loris connection can hold its permit
                // between requests, which is what stops the pool from leaking to zero.
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .header_read_timeout(HTTP_HEADER_READ_TIMEOUT)
                    .serve_connection(io, service)
                    .with_upgrades()
                    .await
                {
                    // Don't log connection reset errors as they're common
                    if !e.to_string().contains("connection reset") {
                        error!("Error serving HTTPS connection: {}", e);
                    }
                }

                // Free the connection slot the instant the HTTP phase ends. For a WebSocket
                // this future resolves at the `with_upgrades()` handoff, after which the
                // socket lives in its own task (see websocket handler) — so an established
                // WebSocket does not occupy one of the HTTPS_MAX_CONCURRENT_CONNECTIONS
                // slots for its lifetime. Dropping explicitly keeps that guarantee from
                // depending on where the enclosing scope happens to end.
                drop(connection_permit);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitized_sentry_path;

    #[test]
    fn sentry_path_redacts_share_tokens() {
        assert_eq!(
            sanitized_sentry_path("/api/v2/requests/shared/secret.jwt"),
            "/api/v2/requests/shared/<redacted>"
        );
        assert_eq!(
            sanitized_sentry_path("/api/v2/requests/123"),
            "/api/v2/requests/123"
        );
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
        // Mark this request as coming over TLS - cannot be spoofed by clients
        req.extensions_mut().insert(TlsConnectInfo);
        self.inner.call(req)
    }
}
