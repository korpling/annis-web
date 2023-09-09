mod auth;
pub mod client;
mod config;
pub mod converter;
pub mod errors;
mod state;
mod views;

use async_sqlx_session::SqliteSessionStore;
use axum::{
    body::{self, Empty, Full},
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_sessions::{async_session::MemoryStore, SameSite, SessionLayer};
use clap::Parser;
use config::CliConfig;
use include_dir::{include_dir, Dir};
use rand::prelude::*;
use state::GlobalAppState;
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use url::Url;

static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");
static TEMPLATES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");

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

fn create_templates(env: &mut minijinja::Environment, frontend_prefix: &str) -> Result<()> {
    // Define any global variables
    env.add_global("url_prefix", frontend_prefix);

    // Load templates by name from the included templates folder
    env.set_loader(|name| {
        if let Some(file) = TEMPLATES_DIR.get_file(name) {
            Ok(file.contents_utf8().map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    });

    Ok(())
}

async fn app(addr: &SocketAddr, service_url: Option<&str>, config: &CliConfig) -> Result<Router> {
    let mut global_state = GlobalAppState::new()?;
    global_state.frontend_prefix = Url::parse(&format!("http://{}", addr))?;
    if let Some(service_url) = service_url {
        global_state.service_url = Url::parse(service_url)?;
    }
    global_state.jwt_type = config.jwt_type()?;

    create_templates(
        &mut global_state.templates,
        global_state.frontend_prefix.as_str(),
    )?;

    let global_state = Arc::new(global_state);

    let routes = Router::new()
        .route("/", get(|| async { Redirect::temporary("corpora") }))
        .route("/static/*path", get(static_file))
        .nest("/corpora", views::corpora::create_routes()?)
        .nest("/export", views::export::create_routes()?)
        .nest("/about", views::about::create_routes()?)
        .nest("/oauth", views::oauth::create_routes()?)
        .with_state(global_state.clone());

    if let Some(session_file) = &config.session_file {
        let store =
            SqliteSessionStore::new(&format!("sqlite://{}", session_file.to_string_lossy()))
                .await?;
        store.migrate().await?;
        store.spawn_cleanup_task(Duration::from_secs(60 * 60));

        let session_layer = SessionLayer::new(
            store.clone(),
            "ginoh3ya5eiLi1nohph0equ6KiwicooweeNgovoojeQuaejaixiequah6eenoo2k".as_bytes(),
        )
        .with_same_site_policy(SameSite::Lax);

        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                global_state.cleanup(&store).await;
            }
        });

        Ok(routes.layer(session_layer))
    } else {
        // TODO remove memory storage option and replace it with a temporary file.
        let store = MemoryStore::new();
        let mut secret = [0_u8; 128];
        rand::thread_rng().fill(&mut secret);
        let session_layer = SessionLayer::new(store, &secret).with_same_site_policy(SameSite::Lax);
        Ok(routes.layer(session_layer))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_str("sqlx::query=warn,graphannis_core=warn,info").unwrap())
        .init();

    let cli = CliConfig::parse();

    let addr = SocketAddr::from(([127, 0, 0, 1], cli.port));

    match app(&addr, None, &cli).await {
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
