use axum::http::HeaderValue;
use axum_sessions::{
    async_session::Session,
    extractors::{ReadableSession, WritableSession},
};
use chrono::Utc;
use oauth2::{basic::BasicClient, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tempfile::NamedTempFile;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use url::Url;

use crate::{auth::LoginInfo, config::CliConfig, Result, TEMPLATES_DIR};

pub const STATE_KEY: &str = "state";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SessionState {
    pub selected_corpora: BTreeSet<String>,
    pub user_name: Option<String>,
    pub login_configured: bool,
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

#[derive(Clone)]
pub enum SessionArg {
    Session(Session),
    Id(String),
}

impl SessionArg {
    pub fn id(&self) -> &str {
        match self {
            SessionArg::Session(s) => s.id(),
            SessionArg::Id(id) => id,
        }
    }
}

pub struct GlobalAppState {
    pub service_url: Url,
    pub templates: minijinja::Environment<'static>,
    pub oauth2_client: Option<BasicClient>,
    pub background_jobs: dashmap::DashMap<String, ExportJob>,
    pub auth_requests: dashmap::DashMap<String, PkceCodeVerifier>,
    pub login_info: dashmap::DashMap<String, LoginInfo>,
}

impl GlobalAppState {
    pub fn new(config: &CliConfig) -> Result<Self> {
        let oauth2_client = config.create_oauth2_basic_client()?;

        let mut templates = minijinja::Environment::new();

        // Define any global variables
        templates.add_global("url_prefix", config.frontend_prefix.to_string());
        templates.add_global("login_enabled", oauth2_client.is_some());

        // Load templates by name from the included templates folder
        templates.set_loader(|name| {
            if let Some(file) = TEMPLATES_DIR.get_file(name) {
                Ok(file.contents_utf8().map(|s| s.to_string()))
            } else {
                Ok(None)
            }
        });

        let result = Self {
            service_url: Url::parse(&config.service_url)?,
            background_jobs: dashmap::DashMap::new(),
            templates,
            auth_requests: dashmap::DashMap::new(),
            login_info: dashmap::DashMap::new(),
            oauth2_client,
        };
        Ok(result)
    }

    pub fn create_client(&self, session: &SessionArg) -> Result<reqwest::Client> {
        if let SessionArg::Session(session) = session {
            self.login_info.alter(session.id(), |_, mut l| {
                if let (Some(old_expiry), Some(new_expiry)) =
                    (l.user_session_expiry, session.expiry())
                {
                    // Check if the new expiration date is actually longer before replacing it
                    if &old_expiry < new_expiry {
                        l.user_session_expiry = Some(*new_expiry);
                    }
                } else {
                    // Use the new expiration date
                    l.user_session_expiry = session.expiry().cloned();
                }
                l
            });
            self.login_info
                .entry(session.id().to_string())
                .and_modify(|l| {
                    l.user_session_expiry = session.expiry().cloned();
                });
        }

        let mut default_headers = reqwest::header::HeaderMap::new();

        if let Some(login) = &self.login_info.get(session.id()) {
            let value = HeaderValue::from_str(&format!("Bearer {}", login.api_token()))?;
            default_headers.insert(reqwest::header::AUTHORIZATION, value);
        }

        let builder = reqwest::ClientBuilder::new().default_headers(default_headers);

        Ok(builder.build()?)
    }

    /// Cleans up ressources coupled to sessions that are expired or non-existing.
    pub async fn cleanup(&self) {
        self.login_info.retain(|_session_id, login_info| {
            if let Some(expiry) = login_info.user_session_expiry {
                Utc::now() < expiry
            } else {
                true
            }
        });
    }
}
