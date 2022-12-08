use std::sync::Arc;

use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html, response::IntoResponse};

use crate::{state::GlobalAppState, Result};

#[derive(Template)]
#[template(path = "corpora.html")]
struct CorporaViewTemplate {
    corpus_names: Vec<String>,
    url_prefix: String,
}

pub async fn corpora(State(state): State<Arc<GlobalAppState>>) -> Result<impl IntoResponse> {
    let mut corpora: Vec<String> = reqwest::get(state.service_url.join("corpora")?)
        .await?
        .json()
        .await?;
    corpora.sort_unstable_by_key(|k| k.to_lowercase());

    let template = CorporaViewTemplate {
        url_prefix: "/".to_string(),
        corpus_names: corpora,
    };
    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}

#[cfg(test)]
mod tests;
