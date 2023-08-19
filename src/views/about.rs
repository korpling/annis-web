use crate::{
    state::{GlobalAppState, SessionState},
    Result,
};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use axum_sessions::extractors::WritableSession;
use minijinja::context;
use std::sync::Arc;

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new().route("/", get(show));
    Ok(result)
}

async fn show(
    session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state = SessionState::from(&session);

    let html = app_state
        .templates
        .get_template("about.html")?
        .render(context! {
            session => session_state,
            version => env!("CARGO_PKG_VERSION"),
        })?;

    Ok(Html(html))
}
