use graphannis::AnnotationGraph;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};

use crate::{
    errors::AppError,
    state::{GlobalAppState, SessionArg},
    Result,
};

/// Get a sorted list of all corpus names
pub async fn list(session: &SessionArg, state: &GlobalAppState) -> Result<Vec<String>> {
    let client = state.create_client(session)?;
    let request = client.get(state.service_url.join("corpora")?).build()?;
    let mut corpora: Vec<String> = client.execute(request).await?.json().await?;
    corpora.sort_by_key(|k| k.to_lowercase());

    Ok(corpora)
}

#[derive(Serialize, Debug)]
struct SubgraphRequest {
    node_ids: Vec<String>,
    segmentation: Option<String>,
    left: usize,
    right: usize,
}

const QUERY: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');

/// Get the subgraph for a given match
pub async fn subgraph(
    session: &SessionArg,
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
    let client = state.create_client(session)?;

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
        >(response_body.as_bytes(), true, |_| {})?;

        Ok(g)
    } else {
        Err(AppError::Backend {
            status_code: response.status(),
            url: response.url().clone(),
        })
    }
}

#[derive(Serialize)]
struct ComponentsRequest {
    #[serde(rename = "type")]
    ctype: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ComponentResponse {
    #[serde(rename = "type")]
    _ctype: Option<String>,
    name: String,
    layer: String,
}

/// List all segmentions (in addition to the token layer) for a given corpus.
pub async fn segmentations(
    session: &SessionArg,
    corpus: &str,
    state: &GlobalAppState,
) -> Result<Vec<String>> {
    let url = state.service_url.join(&format!(
        "corpora/{}/components",
        utf8_percent_encode(corpus, QUERY)
    ))?;
    let client = state.create_client(session)?;

    let query_params = ComponentsRequest {
        ctype: Some("Ordering".to_string()),
        name: None,
    };

    let request = client
        .request(reqwest::Method::GET, url.clone())
        .query(&query_params)
        .build()?;

    let ordering_components: Vec<ComponentResponse> = client.execute(request).await?.json().await?;
    let result: Vec<String> = ordering_components
        .into_iter()
        .filter(|c| !c.name.is_empty() && c.layer != "annis")
        .map(|c| c.name)
        .collect();
    Ok(result)
}
