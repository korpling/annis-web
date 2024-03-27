use crate::auth::LoginInfo;
use crate::{config::CliConfig, errors::AppError, Result, TEMPLATES_DIR};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use chrono::Utc;
use dashmap::DashMap;
use minijinja::Value;
use oauth2::{basic::BasicClient, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};
use tempfile::NamedTempFile;
use time::OffsetDateTime;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Session {
    selected_corpora: BTreeSet<String>,
    #[serde(skip)]
    session: tower_sessions::Session,
    session_id: String,
}

impl Session {
    pub const SELECTED_CORPORA_KEY: &'static str = "selected_corpora";

    fn update_session(
        session: &tower_sessions::Session,
        selected_corpora: &BTreeSet<String>,
    ) -> Result<()> {
        session.insert(Self::SELECTED_CORPORA_KEY, selected_corpora.clone())?;
        Ok(())
    }

    pub fn set_selected_corpora(&mut self, selected_corpora: BTreeSet<String>) -> Result<()> {
        self.selected_corpora = selected_corpora;
        Self::update_session(&self.session, &self.selected_corpora)?;
        Ok(())
    }

    pub fn selected_corpora(&self) -> &BTreeSet<String> {
        &self.selected_corpora
    }

    pub fn id(&self) -> &str {
        &self.session_id
    }

    pub fn expiration_time(&self) -> Option<OffsetDateTime> {
        self.session.expiration_time()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        req: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let session = tower_sessions::Session::from_request_parts(req, state).await?;
        let selected_corpora: BTreeSet<String> = session
            .get(Session::SELECTED_CORPORA_KEY)?
            .unwrap_or_default();

        Self::update_session(&session, &selected_corpora)?;

        Ok(Self {
            session_id: session.id().to_string(),
            session,
            selected_corpora,
        })
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
                        (l.expires_unix(), session.expiration_time())
                    {
                        // Check if the new expiration date is actually longer before replacing it
                        if old_expiry < new_expiry.unix_timestamp() {
                            l.set_expiration_unix(Some(new_expiry.unix_timestamp()));
                        }
                    } else {
                        // Use the new expiration date
                        l.set_expiration_unix(
                            session.expiration_time().map(|t| t.unix_timestamp()),
                        );
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
            if let Some(expiry) = login_info.expires_unix() {
                Utc::now().timestamp() < expiry
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {

    use crate::config::CliConfig;

    use super::*;

    use oauth2::{basic::BasicTokenType, AccessToken, StandardTokenResponse};

    #[test]
    fn client_access_time_updated_existing() {
        let config = CliConfig::default();
        let state = GlobalAppState::new(&config).unwrap();

        // Create a session that should be updated when accessed
        let now = OffsetDateTime::now_utc();

        // The user session will only expire in 1 day
        let session_expiration = now.checked_add(time::Duration::days(1)).unwrap();
        let raw_session = tower_sessions::Session::new(Some(session_expiration));
        let session_id = raw_session.id().to_string();

        let mut session = Session::default();
        session.session_id = session_id.clone();
        session.session = raw_session;

        let access_token = AccessToken::new("ABC".into());
        let token_response = StandardTokenResponse::new(
            access_token,
            BasicTokenType::Bearer,
            oauth2::EmptyExtraTokenFields {},
        );
        // Simulate an old access to the login info, which would trigger a cleanup
        let expired_login_info =
            LoginInfo::from_token(token_response, Some(now.unix_timestamp() - 1)).unwrap();

        state
            .login_info
            .insert(session.session_id.clone(), expired_login_info.clone());

        let session_arg = SessionArg::Session(session.clone());
        state.create_client(&session_arg).unwrap();
        // The login info expiration time must be updated to match the session
        assert_eq!(
            Some(session_expiration.unix_timestamp()),
            state.login_info.get(&session_id).unwrap().expires_unix()
        );
    }

    #[test]
    fn client_access_time_updated_set_from_session() {
        let config = CliConfig::default();
        let state = GlobalAppState::new(&config).unwrap();

        // Create a session that should be updated when accessed
        let now = OffsetDateTime::now_utc();

        // The user session will only expire in 1 day
        let session_expiration = now.checked_add(time::Duration::days(1)).unwrap();
        let raw_session = tower_sessions::Session::new(Some(session_expiration));
        let session_id = raw_session.id().to_string();

        let mut session = Session::default();
        session.session_id = session_id.clone();
        session.session = raw_session;

        let access_token = AccessToken::new("ABC".into());
        let token_response = StandardTokenResponse::new(
            access_token,
            BasicTokenType::Bearer,
            oauth2::EmptyExtraTokenFields {},
        );
        // Simulate an old access to the login info, which does not have a expiration date
        let expired_login_info = LoginInfo::from_token(token_response, None).unwrap();

        state
            .login_info
            .insert(session.session_id.clone(), expired_login_info.clone());

        let session_arg = SessionArg::Session(session.clone());
        state.create_client(&session_arg).unwrap();
        // The login info expiration time must be updated to match the session
        assert_eq!(
            Some(session_expiration.unix_timestamp()),
            state.login_info.get(&session_id).unwrap().expires_unix()
        );
    }
}
