mod auth;
pub mod client;
pub mod config;
pub mod converter;
pub(crate) mod errors;
pub mod state;
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
use axum_sessions::{async_session::SessionStore, SameSite, SessionLayer};
use config::CliConfig;
use include_dir::{include_dir, Dir};
use state::GlobalAppState;
use std::{sync::Arc, time::Duration};

static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");
static TEMPLATES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");

static FALLBACK_COOKIE_KEY: &[u8] =
    "ginoh3ya5eiLi1nohph0equ6KiwicooweeNgovoojeQuaejaixiequah6eenoo2k".as_bytes();

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

pub async fn app(config: &CliConfig) -> Result<Router> {
    let global_state = GlobalAppState::new(config)?;
    let global_state = Arc::new(global_state);

    let db_uri = if let Some(session_file) = &config.session_file {
        format!("sqlite://{}", session_file.to_string_lossy())
    } else {
        // Fallback to a temporary in-memory Sqlite databse
        "sqlite::memory:".to_string()
    };
    let store = SqliteSessionStore::new(&db_uri).await?;
    store.migrate().await?;
    store.spawn_cleanup_task(Duration::from_secs(60 * 60));

    app_with_state(global_state, store).await
}

async fn app_with_state<S: SessionStore>(
    global_state: Arc<GlobalAppState>,
    session_store: S,
) -> Result<Router> {
    let routes = Router::new()
        .route("/", get(|| async { Redirect::temporary("corpora") }))
        .route("/static/*path", get(static_file))
        .nest("/corpora", views::corpora::create_routes()?)
        .nest("/export", views::export::create_routes()?)
        .nest("/about", views::about::create_routes()?)
        .nest("/oauth", views::oauth::create_routes()?)
        .with_state(global_state.clone());

    let session_layer =
        SessionLayer::new(session_store, FALLBACK_COOKIE_KEY).with_same_site_policy(SameSite::Lax);

    tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            global_state.cleanup().await;
        }
    });

    Ok(routes.layer(session_layer))
}

#[cfg(test)]
pub mod tests;
