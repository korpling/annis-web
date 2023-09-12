use axum_sessions::extractors::ReadableSession;
use graphannis::AnnotationGraph;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::Serialize;

use crate::{
    errors::AppError,
    state::{GlobalAppState, SessionState},
    Result,
};

/// Get a sorted list of all corpus names
pub async fn list(session_id: &str, state: &GlobalAppState) -> Result<Vec<String>> {
    let client = state.create_client(session_id)?;
    let request = client.get(state.service_url.join("corpora")?).build()?;
    let mut corpora: Vec<String> = client.execute(request).await?.json().await?;
    corpora.sort_by_key(|k| k.to_lowercase());

    Ok(corpora)
}

#[derive(Serialize)]
struct SubgraphRequest {
    node_ids: Vec<String>,
    segmentation: Option<String>,
    left: usize,
    right: usize,
}

const QUERY: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');

/// Get the subgraph for a given match
pub async fn subgraph(
    session_id: &str,
    corpus: &str,
    node_ids: Vec<String>,
    segmentation: Option<String>,
    left: usize,
    right: usize,
    state: &GlobalAppState,
) -> Result<AnnotationGraph> {
    let url = state.service_url.join(&format!(
        "corpora/{}/subgraph",
        utf8_percent_encode(corpus, QUERY)
    ))?;
    let client = state.create_client(session_id)?;

    let body = SubgraphRequest {
        node_ids,
        segmentation,
        left,
        right,
    };

    let request = client
        .request(reqwest::Method::POST, url.clone())
        .json(&body)
        .build()?;

    let response = client.execute(request).await?;
    if response.status().is_success() {
        let response_body = response.text().await?;

        let (g, _config) = graphannis_core::graph::serialization::graphml::import::<
            graphannis::model::AnnotationComponentType,
            _,
            _,
        >(response_body.as_bytes(), false, |_| {})?;

        Ok(g)
    } else {
        Err(AppError::Backend {
            status_code: response.status(),
            url: response.url().clone(),
        })
    }
}
