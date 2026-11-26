use requestrepo::run;
use std::process::exit;

#[tokio::main]
async fn main() {
    // Install the rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    dotenvy::dotenv().ok();

    // Initialize Sentry for error tracking and performance monitoring
    let _sentry_guard = sentry::init((
        std::env::var("SENTRY_DSN_BACKEND").ok(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            traces_sample_rate: 0.2, // 20% of requests for performance monitoring
            send_default_pii: false,
            ..Default::default()
        },
    ));

    // RUST_LOG as usual (default "info"). rustls warns about every malformed handshake the
    // internet sends, thousands a day, so it is quiet unless RUST_LOG asks for it by name.
    let mut filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    if !filter.contains("rustls") {
        filter.push_str(",rustls=error");
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    if let Err(e) = run().await {
        eprintln!("Application error: {e}");
        exit(1);
    }
}
