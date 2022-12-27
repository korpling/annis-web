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
