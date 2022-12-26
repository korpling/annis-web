use std::{collections::BTreeSet, sync::Arc};

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Form,
};
use axum_sessions::extractors::WritableSession;
use serde::Deserialize;

use crate::{client::search, state::GlobalAppState, Result};

#[derive(Template)]
#[template(path = "components/corpus_selector.html")]
pub struct CorpusSelectorTemplate {
    pub id: String,
    pub url_prefix: String,
    pub corpus_names: Vec<String>,
    pub selected_corpora: BTreeSet<String>,
    pub filter: String,
}

#[derive(Deserialize, Debug)]
pub struct Params {
    filter: String,
    add_corpus: Option<String>,
}

#[tracing::instrument]
pub async fn post(
    mut session: WritableSession,
    State(state): State<Arc<GlobalAppState>>,
    Form(payload): Form<Params>,
) -> Result<impl IntoResponse> {
    let corpora = search::corpora(state.as_ref()).await?;
    let mut filtered_corpora: Vec<_> = corpora
        .iter()
        .filter(|c| c.to_lowercase().contains(&payload.filter.to_lowercase()))
        .cloned()
        .collect();
    filtered_corpora.sort_by_key(|k| k.to_lowercase());

    let mut selected: BTreeSet<String> = session.get("selected_corpora").unwrap_or_default();

    if let Some(add_corpus) = payload.add_corpus {
        selected.insert(add_corpus);
        session.insert("selected_corpora", &selected);
    }

    let template = CorpusSelectorTemplate {
        corpus_names: filtered_corpora,
        url_prefix: state.frontend_prefix.to_string(),
        id: "corpus-selector".to_string(),
        filter: payload.filter.clone(),
        selected_corpora: selected,
    };

    let html = Html(template.render()?);
    Ok((StatusCode::OK, [("HX-Trigger-After-Swap", "refocus")], html))
}
