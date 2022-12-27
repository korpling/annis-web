use std::collections::BTreeSet;

#[cfg(test)]
use mockito;
use serde::{Deserialize, Serialize};
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
        #[cfg(not(test))]
        // TODO: get this parameter a configuration
        let service_url = "http://localhost:5711/v1/";

        #[cfg(test)]
        let service_url: &str = &mockito::server_url();

        let result = Self {
            service_url: Url::parse(service_url)?,
            // TODO: make this configurable
            frontend_prefix: Url::parse("http://localhost:3000/")?,
        };
        Ok(result)
    }
}
