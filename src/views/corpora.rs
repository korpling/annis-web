use std::sync::Arc;

use axum::{
    extract::State,
    response::Html,
    response::IntoResponse,
    routing::{get, post},
    Form, Router,
};
use minijinja::context;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{
    client::corpora,
    state::{GlobalAppState, SessionArg, SessionState, STATE_KEY},
    Result,
};

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new().route("/", get(show)).route("/", post(update));
    Ok(result)
}

#[derive(Serialize)]
struct Corpus {
    name: String,
    selected: bool,
}

async fn show(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state = SessionState::try_from(&session)?;

    let selected_corpora = session_state.selected_corpora.clone();

    let corpora: Vec<_> = corpora::list(&SessionArg::Session(session.clone()), app_state.as_ref())
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
            session => session_state,
            filter => "",
        })?;

    Ok(Html(html))
}

#[derive(Deserialize, Debug)]
struct Params {
    filter: String,
    add_corpus: Option<String>,
    remove_corpus: Option<String>,
    add_all_corpora: Option<String>,
    clear_selection: Option<String>,
}

async fn update(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(payload): Form<Params>,
) -> Result<impl IntoResponse> {
    let mut session_state = SessionState::try_from(&session)?;

    let corpora = corpora::list(&SessionArg::Session(session.clone()), app_state.as_ref()).await?;
    let mut filtered_corpora: Vec<_> = corpora
        .iter()
        .filter(|c| c.to_lowercase().contains(&payload.filter.to_lowercase()))
        .cloned()
        .collect();
    filtered_corpora.sort_by_key(|k| k.to_lowercase());

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

    if payload.clear_selection == Some("true".to_string()) {
        // Unselect all corpora
        session_state.selected_corpora.clear();
    }

    // Update the session
    session.insert(STATE_KEY, session_state.clone())?;

    let corpora: Vec<Corpus> = filtered_corpora
        .into_iter()
        .map(|name| Corpus {
            selected: session_state.selected_corpora.contains(&name),
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
        })?;

    Ok(Html(html))
}

#[cfg(test)]
mod tests;
