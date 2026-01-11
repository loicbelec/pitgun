use std::net::SocketAddr;

use axum::{Router, http::StatusCode, routing::get};
use tokio::net::TcpListener;
use tracing::info;

async fn healthz() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let app = Router::new().route("/healthz", get(healthz));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;

    info!("pitgun-configd listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
