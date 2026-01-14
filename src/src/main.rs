use requestrepo::run;
use std::process::exit;

#[tokio::main]
async fn main() {
    // Install the rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    if let Err(e) = run().await {
        eprintln!("Application error: {e}");
        exit(1);
    }
}
