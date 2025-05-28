use requestrepo::run;
use std::process::exit;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    if let Err(e) = run().await {
        eprintln!("Application error: {}", e);
        exit(1);
    }
}
