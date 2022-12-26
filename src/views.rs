use std::sync::Arc;

use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html, response::IntoResponse};

use crate::{
    client::search, components::corpus_selector::CorpusSelectorTemplate, state::GlobalAppState,
    Result,
};

#[derive(Template)]
#[template(path = "corpora.html")]
struct CorporaViewTemplate {
    url_prefix: String,
    corpus_selector: CorpusSelectorTemplate,
}

#[tracing::instrument]
pub async fn corpora(State(state): State<Arc<GlobalAppState>>) -> Result<impl IntoResponse> {
    let corpora = search::corpora(state.as_ref()).await?;
    let template = CorporaViewTemplate {
        url_prefix: state.frontend_prefix.to_string(),
        corpus_selector: CorpusSelectorTemplate {
            corpus_names: corpora,
            url_prefix: state.frontend_prefix.to_string(),
            id: "corpus-selector".to_string(),
            filter: String::default(),
        },
    };
    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}

#[cfg(test)]
mod tests;
