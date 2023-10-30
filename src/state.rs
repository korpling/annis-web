use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use chrono::Utc;
use dashmap::DashMap;
use minijinja::Value;
use oauth2::{basic::BasicClient, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use tower_sessions::Session;
use url::Url;

use crate::{auth::LoginInfo, config::CliConfig, errors::AppError, Result, TEMPLATES_DIR};

pub const STATE_KEY: &str = "state";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SessionState {
    pub selected_corpora: BTreeSet<String>,
    pub session_id: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for SessionState
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        req: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(req, state).await?;
        let mut state: SessionState = session.get(STATE_KEY)?.unwrap_or_default();
        state.session_id = session.id().to_string();
        session.insert(STATE_KEY, state.clone())?;

        Ok(state)
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
    pub fn id(&self) -> String {
        match self {
            SessionArg::Session(s) => s.id().to_string(),
            SessionArg::Id(id) => id.to_string(),
        }
    }
}

pub struct GlobalAppState {
    pub service_url: Url,
    pub templates: minijinja::Environment<'static>,
    pub oauth2_client: Option<BasicClient>,
    pub background_jobs: DashMap<String, ExportJob>,
    pub auth_requests: DashMap<String, PkceCodeVerifier>,
    pub login_info: Arc<DashMap<String, LoginInfo>>,
    default_client: reqwest::Client,
}

impl GlobalAppState {
    pub fn new(config: &CliConfig) -> Result<Self> {
        let oauth2_client = config.create_oauth2_basic_client()?;

        let mut templates = minijinja::Environment::new();

        // Define any global variables
        templates.add_global("url_prefix", config.frontend_prefix.to_string());
        templates.add_global("login_configured", oauth2_client.is_some());

        // Load templates by name from the included templates folder
        templates.set_loader(|name| {
            if let Some(file) = TEMPLATES_DIR.get_file(name) {
                Ok(file.contents_utf8().map(|s| s.to_string()))
            } else {
                Ok(None)
            }
        });

        let login_info: DashMap<String, LoginInfo> = DashMap::new();
        let login_info = Arc::new(login_info);

        // Add a function for the template that allows to easily extract the username
        let login_info_for_template = login_info.clone();
        templates.add_function("username", move |session: Value| -> Value {
            if let Ok(session_id) = session.get_attr("session_id") {
                if let Some(l) = login_info_for_template.get(&session_id.to_string()) {
                    if let Ok(Some(username)) = l.user_id() {
                        return Value::from(username);
                    }
                }
            }
            Value::UNDEFINED
        });

        let service_url = if config.service_url.is_empty() {
            Url::parse("http://127.0.0.1:5711")?
        } else {
            Url::parse(&config.service_url)?
        };
        let default_client = reqwest::ClientBuilder::new().build()?;
        let result = Self {
            service_url,
            background_jobs: DashMap::new(),
            templates,
            auth_requests: DashMap::new(),
            login_info,
            oauth2_client,
            default_client,
        };
        Ok(result)
    }

    pub fn create_client(&self, session: &SessionArg) -> Result<reqwest::Client> {
        if let SessionArg::Session(session) = session {
            // Mark this login info as accessed, so we know it is not stale and should not be removed
            self.login_info
                .alter(&session.id().to_string(), |_, mut l| {
                    if let (Some(old_expiry), Some(new_expiry)) =
                        (l.user_session_expiry, session.expiration_time())
                    {
                        // Check if the new expiration date is actually longer before replacing it
                        if old_expiry < new_expiry.unix_timestamp() {
                            l.user_session_expiry = Some(new_expiry.unix_timestamp());
                        }
                    } else {
                        // Use the new expiration date
                        l.user_session_expiry =
                            session.expiration_time().map(|t| t.unix_timestamp());
                    }
                    l
                });
        }

        if let Some(login) = &self.login_info.get(&session.id()) {
            // Return the authentifacted client
            Ok(login.get_client())
        } else {
            // Fallback to the default client
            Ok(self.default_client.clone())
        }
    }

    /// Cleans up ressources coupled to sessions that are expired or non-existing.
    pub async fn cleanup(&self) {
        self.login_info.retain(|_session_id, login_info| {
            if let Some(expiry) = login_info.user_session_expiry {
                Utc::now().timestamp() < expiry
            } else {
                true
            }
        });
    }
}
