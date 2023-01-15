use std::sync::Arc;

use crate::{
    client::search::FindQuery,
    converter::CSVExporter,
    state::{GlobalAppState, SessionState},
    Result,
};
use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use axum_sessions::extractors::WritableSession;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};

#[derive(Template, Debug)]
#[template(path = "export.html")]
struct Export {
    url_prefix: String,
    example: String,
    state: SessionState,
}

pub async fn get(
    session: WritableSession,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let example_query = FindQuery {
        query: "tok".to_string(),
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };

    let mut template = Export {
        url_prefix: state.frontend_prefix.to_string(),
        example: "".to_string(),
        state: session_state,
    };

    let mut exporter = CSVExporter::new(example_query);
    let mut example_string_buffer = Vec::new();
    exporter
        .convert_text(state.as_ref(), Some(3), &mut example_string_buffer)
        .await?;

    template.example = String::from_utf8_lossy(&example_string_buffer).to_string();

    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}
