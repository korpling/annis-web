use base64::Engine;
use chrono::{DateTime, Utc};
use oauth2::{basic::BasicTokenType, EmptyExtraTokenFields, StandardTokenResponse, TokenResponse};
use serde::{Deserialize, Serialize};

use crate::{errors::AppError, Result};

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
