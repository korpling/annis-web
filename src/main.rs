pub mod client;
pub mod converter;
pub mod errors;
pub mod state;
mod views;

use async_sqlx_session::SqliteSessionStore;
use axum::{
    body::{self, Empty, Full},
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use clap::Parser;
use include_dir::{include_dir, Dir};
use rand::prelude::*;
use state::GlobalAppState;
use std::{net::SocketAddr, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use url::Url;
static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");

pub type Result<T> = std::result::Result<T, errors::AppError>;

async fn static_file(Path(path): Path<String>) -> Result<impl IntoResponse> {
    let path = path.trim_start_matches('/');
    let mime_type = mime_guess::from_path(path).first_or_text_plain();

    let response = match STATIC_DIR.get_file(path) {
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(body::boxed(Empty::new()))?,
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime_type.as_ref()).unwrap(),
            )
            .body(body::boxed(Full::from(file.contents())))?,
    };
    Ok(response)
}

async fn app(
    addr: &SocketAddr,
    service_url: Option<&str>,
    session_file: Option<&std::path::Path>,
) -> Result<Router> {
    let mut global_state = GlobalAppState::new()?;
    global_state.frontend_prefix = Url::parse(&format!("http://{}", addr))?;
    if let Some(service_url) = service_url {
        global_state.service_url = Url::parse(service_url)?;
    }

    let result = Router::new()
        .route("/", get(views::corpora::get))
        .route("/", post(views::corpora::post))
        .route("/export", get(views::export::show_page))
        .route("/export/job", post(views::export::create_job))
        .route("/export/job", get(views::export::job_status))
        .route("/export/file", get(views::export::download_file))
        .route("/static/*path", get(static_file))
        .with_state(Arc::new(global_state));

    if let Some(session_file) = session_file {
        let store =
            SqliteSessionStore::new(&format!("sqlite://{}", session_file.to_string_lossy()))
                .await?;
        store.migrate().await?;
        store.spawn_cleanup_task(Duration::from_secs(60 * 60));
        let session_layer = SessionLayer::new(
            store,
            "ginoh3ya5eiLi1nohph0equ6KiwicooweeNgovoojeQuaejaixiequah6eenoo2k".as_bytes(),
        );
        Ok(result.layer(session_layer))
    } else {
        let store = MemoryStore::new();
        let mut secret = [0_u8; 128];
        rand::thread_rng().fill(&mut secret);
        let session_layer = SessionLayer::new(store, &secret).with_secure(false);
        Ok(result.layer(session_layer))
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Port to listen to
    #[arg(long, short, default_value_t = 3000)]
    port: u16,
    /// If set, the SQLite database file to store sessions in
    #[arg(long)]
    session_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_str("sqlx::query=warn,graphannis_core=warn,info").unwrap())
        .init();

    let cli = Cli::parse();

    let addr = SocketAddr::from(([127, 0, 0, 1], cli.port));

    match app(&addr, None, cli.session_file.as_deref()).await {
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

#[cfg(test)]
pub mod tests;
