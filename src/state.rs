use axum::http::HeaderValue;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use jsonwebtoken::DecodingKey;
use oauth2::PkceCodeVerifier;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tempfile::NamedTempFile;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use url::Url;

use crate::{auth::LoginInfo, Result};

pub const STATE_KEY: &str = "state";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SessionState {
    pub selected_corpora: BTreeSet<String>,
    pub login: Option<LoginInfo>,
}

impl From<&ReadableSession> for SessionState {
    fn from(value: &ReadableSession) -> Self {
        value.get(STATE_KEY).unwrap_or_default()
    }
}

impl From<&WritableSession> for SessionState {
    fn from(value: &WritableSession) -> Self {
        value.get(STATE_KEY).unwrap_or_default()
    }
}

impl SessionState {
    pub fn create_client(&self) -> Result<reqwest::Client> {
        let mut default_headers = reqwest::header::HeaderMap::new();

        if let Some(login) = &self.login {
            let value = HeaderValue::from_str(&format!("Bearer {}", login.api_token()))?;
            default_headers.insert(reqwest::header::AUTHORIZATION, value);
        }

        let builder = reqwest::ClientBuilder::new().default_headers(default_headers);

        Ok(builder.build()?)
    }
}

#[derive(Debug)]
pub struct ExportJob {
    pub handle: JoinHandle<Result<NamedTempFile>>,
    progress: f32,
    progress_receiver: Receiver<f32>,
}

impl ExportJob {
    pub fn new(
        handle: JoinHandle<Result<NamedTempFile>>,
        progress_receiver: Receiver<f32>,
    ) -> ExportJob {
        ExportJob {
            handle,
            progress_receiver,
            progress: 0.0,
        }
    }

    pub fn get_progress(&mut self) -> f32 {
        while let Ok(new_progress) = self.progress_receiver.try_recv() {
            self.progress = new_progress;
        }
        self.progress
    }
}

pub enum JwtType {
    None,
    HS256(DecodingKey),
    RS256(DecodingKey),
}

pub struct GlobalAppState {
    pub service_url: Url,
    pub frontend_prefix: Url,
    pub templates: minijinja::Environment<'static>,
    pub background_jobs: dashmap::DashMap<String, ExportJob>,
    pub auth_requests: dashmap::DashMap<String, PkceCodeVerifier>,
    pub jwt_type: JwtType,
}

impl GlobalAppState {
    pub fn new() -> Result<Self> {
        // TODO: get this parameter a configuration
        let service_url = "http://localhost:5711/v1/";

        let result = Self {
            service_url: Url::parse(service_url)?,
            // TODO: make this configurable
            frontend_prefix: Url::parse("http://localhost:3000/")?,
            background_jobs: dashmap::DashMap::new(),
            templates: minijinja::Environment::new(),
            auth_requests: dashmap::DashMap::new(),
            jwt_type: JwtType::None,
        };
        Ok(result)
    }
}
