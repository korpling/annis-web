use annis_web::{app, config::CliConfig};
use chrono::Duration;
use clap::Parser;
use std::{net::SocketAddr, str::FromStr};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_str("sqlx::query=warn,graphannis_core=warn,info").unwrap())
        .init();

    let cli = CliConfig::parse();

    let addr = SocketAddr::from(([127, 0, 0, 1], cli.port));
    match app(&cli, Duration::hours(1)).await {
        Ok(router) => {
            info!("Starting server with address http://{addr}", addr = addr);
            let server = axum::Server::bind(&addr).serve(router.into_make_service());
            if let Err(e) = server.await {
                error!("{}", e);
            }
        }
        Err(e) => {
            error!("Could not initialize server. {}", e);
        }
    }
}
