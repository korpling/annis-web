use graphannis::AnnotationGraph;
use serde::Serialize;

use crate::{state::GlobalAppState, Result};

/// Get a sorted list of all corpus names
pub async fn list(state: &GlobalAppState) -> Result<Vec<String>> {
    let mut corpora: Vec<String> = reqwest::get(state.service_url.join("corpora")?)
        .await?
        .json()
        .await?;
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

/// Get the subgraph for a given match
pub async fn subgraph(
    corpus: &str,
    node_ids: Vec<String>,
    segmentation: Option<String>,
    left: usize,
    right: usize,
    state: &GlobalAppState,
) -> Result<AnnotationGraph> {
    let url = state.service_url.join("corpus")?;
    let url = url.join(corpus)?;
    let client = reqwest::Client::builder().build()?;

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

    let response = client.execute(request).await?.bytes().await?;

    let (g, _config) = graphannis_core::graph::serialization::graphml::import::<
        graphannis::model::AnnotationComponentType,
        _,
        _,
    >(&response[..], false, |_| {})?;

    Ok(g)
}
