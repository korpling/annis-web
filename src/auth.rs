use std::{sync::Arc, time::Duration};

use base64::Engine;
use chrono::{DateTime, Utc};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    reqwest::async_http_client,
    EmptyExtraTokenFields, RefreshToken, StandardTokenResponse, TokenResponse,
};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{debug, warn};

use crate::{errors::AppError, state::GlobalAppState, Result};

pub type AnnisTokenResponse = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginInfo {
    oauth_token: AnnisTokenResponse,

    /// Date and time when the session attached to this login information expires.
    pub user_session_expiry: Option<DateTime<Utc>>,
}

fn parse_unverified_username(token: &str) -> Result<Option<String>> {
    let splitted: Vec<_> = token.splitn(3, '.').collect();

    if let Some(raw_claims) = splitted.get(1) {
        let claims_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw_claims)?;
        let claims: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&claims_json))?;
        let mut user_name = None;
        if let Some(claims) = claims.as_object() {
            if let Some(preferred_username) = claims.get("preferred_username") {
                user_name = preferred_username.as_str();
            } else if let Some(sub) = claims.get("sub") {
                user_name = sub.as_str();
            }
        }
        Ok(user_name.map(str::to_string))
    } else {
        Err(AppError::JwtMissingPayload)
    }
}

impl LoginInfo {
    pub fn new(
        oauth_token: AnnisTokenResponse,
        user_session_expiry: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        let result = LoginInfo {
            oauth_token,
            user_session_expiry,
        };
        Ok(result)
    }

    pub fn renew_token(&mut self, oauth_token: AnnisTokenResponse) -> Result<()> {
        self.oauth_token = oauth_token;
        Ok(())
    }

    pub fn api_token(&self) -> &str {
        self.oauth_token.access_token().secret()
    }

    pub fn user_id(&self) -> Result<Option<String>> {
        // We do not store any information or provide access to resources,
        // but just display whatever ID the user claims to have. Validation is
        // performed in the backend, so we can ignore the signature here.
        parse_unverified_username(self.api_token())
    }
}

async fn refresh_token_action(
    refresh_instant: Instant,
    refresh_token: RefreshToken,
    client: BasicClient,
    session_id: String,
    app_state: Arc<GlobalAppState>,
    margin: Duration,
) -> Result<()> {
    debug!(
        "Waiting to refresh token in background for session {}",
        &session_id
    );
    tokio::time::sleep_until(refresh_instant).await;

    let token_request_time = Instant::now();
    let new_token = client
        .exchange_refresh_token(&refresh_token)
        .request_async(async_http_client)
        .await?;

    debug!("Refreshed client token for session {}", &session_id);

    // Re-use the user session expiration date of the previous login info. The user
    // session experiation should be updated whenever the user actually accesses
    // our server. If they stop to access it, we should not attempt to renew the
    // access token in the background.
    if let Some(mut login_info) = app_state.login_info.get_mut(&session_id) {
        match login_info.renew_token(new_token.clone()) {
            Ok(_) => {
                // Schedule a new token refresh if the for when the new token expires
                schedule_refresh_token(
                    &new_token,
                    client,
                    &session_id,
                    token_request_time,
                    app_state.clone(),
                    margin,
                )
            }
            Err(e) => {
                warn!("Could not renew-token for session {session_id}: {e}");
            }
        }
    }
    Ok(())
}

/// Schedule a task that will be executed to refresh the token shortly before it expires.
pub fn schedule_refresh_token(
    token: &AnnisTokenResponse,
    client: BasicClient,
    session_id: &str,
    token_request_time: Instant,
    app_state: Arc<GlobalAppState>,
    margin: Duration,
) {
    if let (Some(expires_in), Some(refresh_token)) =
        (token.expires_in(), token.refresh_token().cloned())
    {
        let refresh_offset = expires_in.checked_sub(margin).unwrap_or(expires_in);
        let refresh_instant = token_request_time.checked_add(refresh_offset);
        let session_id = session_id.to_string();
        if let Some(refresh_instant) = refresh_instant {
            tokio::spawn(refresh_token_action(
                refresh_instant,
                refresh_token,
                client,
                session_id,
                app_state,
                margin,
            ));
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::config::CliConfig;

    use super::*;
    use std::collections::HashMap;

    use mockito::Server;
    use oauth2::AccessToken;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_refresh_token() {
        // Create a mock auth server, that always returns a new very short-lifed
        // JWT token when requested
        let mut oauth_server = Server::new();
        let mut mock_token_response: HashMap<&str, serde_json::Value> = HashMap::new();
        mock_token_response.insert("access_token", "refreshed-token".into());
        mock_token_response.insert("token_type", "Bearer".into());
        // This special token expires in only 1 second
        mock_token_response.insert("expires_in", 1.into());
        mock_token_response.insert("scope", "".into());

        let mock_token_request = oauth_server
            .mock("POST", "/token")
            .with_body(serde_json::to_string(&mock_token_response).unwrap())
            .with_header("Content-Type", "application/json")
            .expect(1)
            .create();

        let mut config = CliConfig::default();
        config.oauth2_auth_url = Some(format!("{}/auth", oauth_server.url()));
        config.oauth2_token_url = Some(format!("{}/token", oauth_server.url()));

        let app_state = Arc::new(GlobalAppState::new(&config).unwrap());

        let client = app_state.oauth2_client.as_ref().unwrap();

        let session_id = "not-a-real-session-id";
        let mut token = AnnisTokenResponse::new(
            AccessToken::new("original-token".into()),
            BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        token.set_expires_in(Some(&Duration::from_secs(1)));
        let refresh_token = RefreshToken::new("not-a-real-refresh-token".into());
        token.set_refresh_token(Some(refresh_token));

        app_state.login_info.insert(
            session_id.to_string(),
            LoginInfo::new(token.clone(), None).unwrap(),
        );

        let token_request_time = Instant::now();
        schedule_refresh_token(
            &token,
            client.clone(),
            session_id,
            token_request_time,
            app_state.clone(),
            Duration::from_millis(100),
        );

        // Wait at least the expiration time and make sure the token was
        // requested from the mock server
        tokio::time::sleep(Duration::from_secs(1)).await;
        mock_token_request.assert();
        // The token stored in the state must be updated
        let login_info = app_state.login_info.get(session_id).unwrap();
        assert_eq!(login_info.api_token(), "refreshed-token");
    }
}
