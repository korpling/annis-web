mod auth;
pub mod client;
pub mod config;
pub mod converter;
pub(crate) mod errors;
pub mod state;
mod views;

use axum::{
    body::{self, Empty, Full},
    error_handling::HandleErrorLayer,
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::{IntoResponse, Redirect},
    routing::get,
    BoxError, Router,
};
use config::CliConfig;
use include_dir::{include_dir, Dir};
use state::GlobalAppState;
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;
use tower_sessions::{cookie::SameSite, sqlx::SqlitePool, SessionManagerLayer, SqliteStore};

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

pub async fn app(config: &CliConfig) -> Result<Router> {
    let global_state = GlobalAppState::new(config)?;
    let global_state = Arc::new(global_state);

    let db_uri = if let Some(session_file) = &config.session_file {
        format!("sqlite://{}", session_file.to_string_lossy())
    } else {
        // Fallback to a temporary in-memory Sqlite databse
        "sqlite::memory:".to_string()
    };
    let db_pool = SqlitePool::connect(&db_uri).await?;
    let store = SqliteStore::new(db_pool);
    store.migrate().await?;

    tokio::task::spawn(
        store
            .clone()
            .continuously_delete_expired(Duration::from_secs(60 * 60)),
    );

    app_with_state(global_state, store).await
}

async fn app_with_state(
    global_state: Arc<GlobalAppState>,
    session_store: SqliteStore,
) -> Result<Router> {
    let routes = Router::new()
        .route("/", get(|| async { Redirect::temporary("corpora") }))
        .route("/static/*path", get(static_file))
        .nest("/corpora", views::corpora::create_routes()?)
        .nest("/export", views::export::create_routes()?)
        .nest("/about", views::about::create_routes()?)
        .nest("/oauth", views::oauth::create_routes()?)
        .with_state(global_state.clone());

    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax));

    tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            global_state.cleanup().await;
        }
    });

    Ok(routes.layer(session_service))
}

#[cfg(test)]
pub mod tests;
