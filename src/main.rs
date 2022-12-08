pub mod components;
pub mod errors;
pub mod state;
mod views;

use axum::{
    body::{self, Empty, Full},
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use include_dir::{include_dir, Dir};
use state::GlobalAppState;
use std::{net::SocketAddr, sync::Arc};
use tracing::{error, info};

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

fn app() -> Result<Router> {
    let global_state = Arc::new(GlobalAppState::new()?);

    let result = Router::new()
        .route("/", get(views::corpora))
        .route("/static/*path", get(static_file))
        .with_state(global_state);
    Ok(result)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    match app() {
        Ok(router) => {
            let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
            info!("Starting server with address {addr}", addr = addr);
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
