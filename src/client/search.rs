use axum::http::StatusCode;
use futures::TryStreamExt;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use serde::{Deserialize, Serialize};
use std::{io::ErrorKind, mem::size_of};
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;
use tracing::error;
use transient_btree_index::{BtreeConfig, BtreeIndex};

use crate::{
    errors::{AppError, BadRequestError},
    state::{GlobalAppState, SessionArg},
    Result,
};

#[derive(Serialize, Clone)]
pub struct FindQuery {
    pub query: String,
    pub corpora: Vec<String>,
    pub query_language: QueryLanguage,
    pub limit: Option<u64>,
    pub order: ResultOrder,
}

/// Find all matches for a given query
pub async fn find(
    session: &SessionArg,
    query: &FindQuery,
    state: &GlobalAppState,
) -> Result<BtreeIndex<u64, Vec<String>>> {
    let url = state.service_url.join("search/find")?;
    let client = state.create_client(session)?;

    let request = client
        .request(reqwest::Method::POST, url.clone())
        .json(&query)
        .build()?;

    let response = client.execute(request).await?;

    if response.status().is_success() {
        let response = response.bytes_stream();

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
    } else if response.status() == StatusCode::BAD_REQUEST {
        let original_error: BadRequestError = response.json().await?;
        Err(AppError::BackendBadRequest(original_error))
    } else {
        Err(AppError::Backend {
            status_code: response.status(),
            url,
        })
    }
}

#[derive(Serialize)]
struct NodeDescriptionsRequest {
    query: String,
    query_language: QueryLanguage,
}

#[derive(Deserialize, Debug)]
pub struct NodeDescriptionsResponse {
    pub alternative: usize,
    pub query_fragment: String,
    pub variable: String,
    pub anno_name: Option<String>,
    pub optional: Option<bool>,
}

/// Parses a query on the server and returns a description for all the nodes in the query.
pub async fn node_descriptions(
    session: &SessionArg,
    query: &str,
    query_language: QueryLanguage,
    state: &GlobalAppState,
) -> Result<Vec<NodeDescriptionsResponse>> {
    let url = state.service_url.join("search/node-descriptions")?;
    let client = state.create_client(session)?;

    let params = NodeDescriptionsRequest {
        query: query.into(),
        query_language,
    };

    let request = client
        .request(reqwest::Method::GET, url.clone())
        .query(&params)
        .build()?;

    let response = client.execute(request).await?;
    if response.status().is_success() {
        let result: Vec<NodeDescriptionsResponse> = response.json().await?;
        Ok(result)
    } else if response.status() == StatusCode::BAD_REQUEST {
        let original_error: BadRequestError = response.json().await?;
        Err(AppError::BackendBadRequest(original_error))
    } else {
        Err(AppError::Backend {
            status_code: response.status(),
            url,
        })
    }
}
