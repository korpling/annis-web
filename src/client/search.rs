use futures::TryStreamExt;
use serde::Serialize;
use std::{io::ErrorKind, mem::size_of};
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;
use tracing::error;
use transient_btree_index::{BtreeConfig, BtreeIndex};

use crate::{state::GlobalAppState, Result};

/// Get a sorted list of all corpus names
pub async fn corpora(state: &GlobalAppState) -> Result<Vec<String>> {
    let mut corpora: Vec<String> = reqwest::get(state.service_url.join("corpora")?)
        .await?
        .json()
        .await?;
    corpora.sort_by_key(|k| k.to_lowercase());

    Ok(corpora)
}

#[derive(Serialize)]
struct FindRequest {
    query: String,
    corpora: Vec<String>,
    limit: u64,
}

/// Find all matches for a given query
pub async fn find(
    aql: &str,
    corpora: Vec<String>,
    limit: Option<u64>,
    state: &GlobalAppState,
) -> Result<BtreeIndex<u64, Vec<String>>> {
    let url = state.service_url.join("search/find")?;
    let client = reqwest::Client::builder().build()?;

    let body = FindRequest {
        corpora,
        limit: limit.unwrap_or(u64::MAX),
        query: aql.to_string(),
    };

    let request = client
        .request(reqwest::Method::POST, url.clone())
        .json(&body)
        .build()?;

    let response = client.execute(request);

    let response = response.await?.bytes_stream();

    // Each line is a match, go through the body of the response and collect the matches
    let mut result = BtreeIndex::with_capacity(
        BtreeConfig::default().fixed_key_size(size_of::<u64>()),
        1024,
    )?;
    let mut lines = StreamReader::new(response.map_err(|e| -> std::io::Error {
        error!("Could not get next matches for find query. {}", e);
        ErrorKind::ConnectionAborted.into()
    }))
    .lines();
    let mut i = 0;
    while let Some(l) = lines.next_line().await? {
        result.insert(i, graphannis::util::node_names_from_match(&l))?;
        i += 1;
    }

    Ok(result)
}
