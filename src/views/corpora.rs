use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::Html,
    response::{IntoResponse, Response},
    Form,
};
use axum_sessions::extractors::WritableSession;
use minijinja::context;
use serde::{Deserialize, Serialize};

use crate::{
    client::corpora,
    state::{GlobalAppState, SessionState},
    Result,
};

#[derive(Serialize)]
struct Corpus {
    name: String,
    selected: bool,
}

pub async fn get(
    session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let selected_corpora = session_state.selected_corpora.clone();

    let corpora: Vec<_> = corpora::list(app_state.as_ref())
        .await?
        .into_iter()
        .map(|name| Corpus {
            selected: selected_corpora.contains(&name),
            name,
        })
        .collect();

    let html = app_state
        .templates
        .get_template("corpora.html")?
        .render(context! {
            corpora,
            selected_corpora,
            session => session_state,
        })?;

    Ok(Html(html))
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

    let selected_corpora = session_state.selected_corpora.clone();
    let corpora: Vec<_> = filtered_corpora
        .into_iter()
        .map(|name| Corpus {
            selected: selected_corpora.contains(&name),
            name,
        })
        .collect();

    let html = app_state
        .templates
        .get_template("corpora.html")?
        .render(context! {
            corpora,
            filter => payload.filter,
            session => session_state,
            selected_corpora,
        })?;

    Ok((StatusCode::OK, [("HX-Trigger-After-Swap", "refocus")], html).into_response())
}

#[cfg(test)]
mod tests;
