use std::sync::Arc;

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use minijinja::context;

use crate::{
    state::{GlobalAppState, Session},
    Result,
};

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new().route("/", get(show_page));

    Ok(result)
}

async fn show_page(
    session: Session,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let result = state
        .templates
        .get_template("frequency.html")?
        .render(context! {
            session => session,
        })?;

    Ok(Html(result))
}
