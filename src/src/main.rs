use requestrepo::run;
use std::process::exit;

#[tokio::main]
async fn main() {
    // Install the rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    dotenv::dotenv().ok();

    // Initialize Sentry for error tracking and performance monitoring
    let _sentry_guard = sentry::init((
        std::env::var("SENTRY_DSN_BACKEND").ok(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            traces_sample_rate: 0.2, // 20% of requests for performance monitoring
            send_default_pii: true,
            ..Default::default()
        },
    ));

    tracing_subscriber::fmt::init();

    if let Err(e) = run().await {
        eprintln!("Application error: {e}");
        exit(1);
    }
}
