use std::sync::Arc;

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use graphannis::corpusstorage::FrequencyDefEntry;
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
    let mut freq_def = Vec::new();
    freq_def.push(FrequencyDefEntry {
        ns: None,
        name: "pos".into(),
        node_ref: "1".into(),
    });
    freq_def.push(FrequencyDefEntry {
        ns: None,
        name: "tok".into(),
        node_ref: "2".into(),
    });
    let result = state
        .templates
        .get_template("frequency.html")?
        .render(context! {
            session => session,
            freq_def => freq_def,
        })?;

    Ok(Html(result))
}
