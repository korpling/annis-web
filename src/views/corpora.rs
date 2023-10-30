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

use crate::{
    client::corpora,
    state::{GlobalAppState, Session, SessionArg},
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
    let selected_corpora = &session.selected_corpora();

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
            session => session,
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
    mut session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(payload): Form<Params>,
) -> Result<impl IntoResponse> {
    let corpora = corpora::list(&SessionArg::Session(session.clone()), app_state.as_ref()).await?;
    let mut filtered_corpora: Vec<_> = corpora
        .iter()
        .filter(|c| c.to_lowercase().contains(&payload.filter.to_lowercase()))
        .cloned()
        .collect();
    filtered_corpora.sort_by_key(|k| k.to_lowercase());

    let mut selected_corpora = session.selected_corpora().clone();

    if let Some(add_corpus) = payload.add_corpus {
        selected_corpora.insert(add_corpus);
    }

    if let Some(remove_corpus) = payload.remove_corpus {
        selected_corpora.remove(&remove_corpus);
    }
    if payload.add_all_corpora == Some("true".to_string()) {
        // Add all the filtered corpora to the selection
        for c in &filtered_corpora {
            selected_corpora.insert(c.clone());
        }
    }

    if payload.clear_selection == Some("true".to_string()) {
        // Unselect all corpora
        selected_corpora.clear();
    }

    // Update the session
    session.set_selected_corpora(selected_corpora)?;

    let corpora: Vec<Corpus> = filtered_corpora
        .into_iter()
        .map(|name| Corpus {
            selected: session.selected_corpora().contains(&name),
            name,
        })
        .collect();

    let html = app_state
        .templates
        .get_template("corpora.html")?
        .render(context! {
            corpora,
            filter => payload.filter,
            session => session,
        })?;

    Ok(Html(html))
}

#[cfg(test)]
mod tests;
