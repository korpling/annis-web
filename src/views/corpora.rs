use std::{collections::BTreeSet, sync::Arc};

use askama::Template;
use axum::{
    extract::State,
    headers::HeaderMap,
    http::StatusCode,
    response::Html,
    response::{IntoResponse, Response},
    Form,
};
use axum_sessions::extractors::WritableSession;
use serde::Deserialize;

use crate::{
    client::corpora,
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

impl Corpora {
    fn new(app_state: &GlobalAppState) -> Self {
        Self {
            id: "corpus-selector".to_string(),
            url_prefix: app_state.frontend_prefix.to_string(),
            corpus_names: Vec::new(),
            selected_corpora: BTreeSet::default(),
            filter: String::default(),
        }
    }
}

#[derive(Template)]
#[template(path = "corpora_full.html")]
struct CorporaFull {
    url_prefix: String,
    inner: Corpora,
    state: SessionState,
}

pub async fn get(
    session: WritableSession,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let corpora = corpora::list(state.as_ref()).await?;
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let mut inner = Corpora::new(state.as_ref());
    inner.corpus_names = corpora;
    inner.selected_corpora = session_state.selected_corpora.clone();

    Ok(CorporaFull {
        url_prefix: state.frontend_prefix.to_string(),
        inner,
        state: session_state,
    })
}

#[derive(Deserialize, Debug)]
pub struct Params {
    filter: String,
    add_corpus: Option<String>,
    remove_corpus: Option<String>,
    add_all_corpora: Option<String>,
}

pub async fn post(
    mut session: WritableSession,
    headers: HeaderMap,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(payload): Form<Params>,
) -> Result<Response> {
    let corpora = corpora::list(app_state.as_ref()).await?;
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
    if let Some(remove_corpus) = payload.remove_corpus {
        session_state.selected_corpora.remove(&remove_corpus);
    }
    if payload.add_all_corpora == Some("true".to_string()) {
        // Add all the filtered corpora to the selection
        for c in &filtered_corpora {
            session_state.selected_corpora.insert(c.clone());
        }
    }

    session.insert("state", session_state.clone())?;

    let mut inner = Corpora::new(app_state.as_ref());
    inner.corpus_names = filtered_corpora;
    inner.filter = payload.filter;
    inner.selected_corpora = session_state.selected_corpora.clone();

    if headers.contains_key("HX-Target") {
        // Only return the part that needs to be re-rendered
        let html = Html(inner.render()?);
        Ok((StatusCode::OK, [("HX-Trigger-After-Swap", "refocus")], html).into_response())
    } else {
        // Return the full site
        let template = CorporaFull {
            inner,
            url_prefix: app_state.frontend_prefix.to_string(),
            state: session_state,
        };
        Ok((StatusCode::OK, template).into_response())
    }
}

#[cfg(test)]
mod tests;
