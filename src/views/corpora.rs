use std::{collections::BTreeSet, sync::Arc};

use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html, response::IntoResponse, Form};
use axum_sessions::extractors::WritableSession;
use serde::Deserialize;

use crate::{
    client::search,
    state::{GlobalAppState, SessionState},
    Result,
};

#[derive(Template)]
#[template(path = "corpora.html")]
pub struct Corpora {
    pub id: String,
    pub url_prefix: String,
    pub corpus_names: Vec<String>,
    pub selected_corpora: BTreeSet<String>,
    pub filter: String,
}

#[derive(Template)]
#[template(path = "corpora_full.html")]
struct CorporaFull {
    pub id: String,
    url_prefix: String,
    inner: Corpora,
}

#[tracing::instrument]
pub async fn get(State(state): State<Arc<GlobalAppState>>) -> Result<impl IntoResponse> {
    let corpora = search::corpora(state.as_ref()).await?;
    let template = CorporaFull {
        url_prefix: state.frontend_prefix.to_string(),
        id: "corpus-view-root".into(),
        inner: Corpora {
            corpus_names: corpora,
            url_prefix: state.frontend_prefix.to_string(),
            id: "corpus-selector".to_string(),
            filter: String::default(),
            selected_corpora: BTreeSet::default(),
        },
    };
    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}

#[derive(Deserialize, Debug)]
pub struct Params {
    filter: String,
    add_corpus: Option<String>,
}

#[tracing::instrument]
pub async fn post(
    mut session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(payload): Form<Params>,
) -> Result<impl IntoResponse> {
    let corpora = search::corpora(app_state.as_ref()).await?;
    let mut filtered_corpora: Vec<_> = corpora
        .iter()
        .filter(|c| c.to_lowercase().contains(&payload.filter.to_lowercase()))
        .cloned()
        .collect();
    filtered_corpora.sort_by_key(|k| k.to_lowercase());

    let mut session_state: SessionState = session.get("state").unwrap_or_default();

    if let Some(add_corpus) = payload.add_corpus {
        session_state.selected_corpora.insert(add_corpus);
    }

    session.insert("state", session_state.clone())?;

    let inner = Corpora {
        corpus_names: filtered_corpora,
        url_prefix: app_state.frontend_prefix.to_string(),
        id: "corpus-selector".to_string(),
        filter: payload.filter,
        selected_corpora: session_state.selected_corpora,
    };

    let template = CorporaFull {
        inner,
        id: "corpus-view-root".into(),
        url_prefix: app_state.frontend_prefix.to_string(),
    };

    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}

#[cfg(test)]
mod tests;
