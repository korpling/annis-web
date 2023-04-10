use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use url::Url;

use crate::Result;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionState {
    pub selected_corpora: BTreeSet<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            selected_corpora: BTreeSet::default(),
        }
    }
}

#[derive(Debug)]
pub struct GlobalAppState {
    pub service_url: Url,
    pub frontend_prefix: Url,
}

impl GlobalAppState {
    pub fn new() -> Result<Self> {
        // TODO: get this parameter a configuration
        let service_url = "http://localhost:5711/v1/";

        let result = Self {
            service_url: Url::parse(service_url)?,
            // TODO: make this configurable
            frontend_prefix: Url::parse("http://localhost:3000/")?,
        };
        Ok(result)
    }
}
