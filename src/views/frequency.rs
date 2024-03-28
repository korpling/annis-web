use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use graphannis::corpusstorage::FrequencyDefEntry;
use minijinja::context;
use serde::Deserialize;

use crate::{
    client::{self},
    state::{GlobalAppState, Session, SessionArg},
    Result,
};

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new().route("/", get(show_page));

    Ok(result)
}

#[derive(Deserialize, Debug)]
struct FormParams {
    query: Option<String>,
}

async fn show_page(
    session: Session,
    State(state): State<Arc<GlobalAppState>>,
    Query(params): Query<FormParams>,
) -> Result<impl IntoResponse> {
    let freq_def = if let Some(query) = params.query {
        if query.is_empty() {
            Ok(Vec::default())
        } else {
            default_frequency_definition(&query, &state, &session).await
        }
    } else {
        Ok(Vec::default())
    };
    let result = state
        .templates
        .get_template("frequency.html")?
        .render(context! {
            session => session,
            freq_def => freq_def,
        })?;

    Ok(Html(result))
}

async fn default_frequency_definition(
    query: &str,
    state: &GlobalAppState,
    session: &Session,
) -> std::result::Result<Vec<FrequencyDefEntry>, String> {
    let descriptions = client::search::node_descriptions(
        &SessionArg::Session(session.clone()),
        &query,
        graphannis::corpusstorage::QueryLanguage::AQL,
        &state,
    )
    .await
    .map_err(|e| format!("{e}"))?;

    let mut result = Vec::with_capacity(descriptions.len());

    for node in descriptions {
        result.push(FrequencyDefEntry {
            ns: None,
            name: node.anno_name.unwrap_or("tok".into()),
            node_ref: node.variable,
        });
    }

    Ok(result)
}
