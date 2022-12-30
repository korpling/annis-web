use std::sync::Arc;

use crate::{converter::CSVExporter, state::GlobalAppState, Result};
use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};

#[derive(Template, Debug)]
#[template(path = "export.html")]
struct Export {
    url_prefix: String,
    example: String,
}

pub async fn get(State(state): State<Arc<GlobalAppState>>) -> Result<impl IntoResponse> {
    let mut template = Export {
        url_prefix: state.frontend_prefix.to_string(),
        example: "".to_string(),
    };

    let exporter = CSVExporter::new("tok");
    let mut example_string_buffer = Vec::new();
    exporter
        .convert_text(state.as_ref(), Some(3), &mut example_string_buffer)
        .await?;

    template.example = String::from_utf8_lossy(&example_string_buffer).to_string();

    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}
