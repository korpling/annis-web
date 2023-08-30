use oauth2::{basic::BasicTokenType, EmptyExtraTokenFields, StandardTokenResponse, TokenResponse};
use serde::{Deserialize, Serialize};

use crate::{errors::AppError, state::JwtType, Result};

pub type AnnisTokenResponse = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginInfo {
    oauth_token: AnnisTokenResponse,
    pub claims: Claims,
}

impl LoginInfo {
    pub fn new(oauth_token: AnnisTokenResponse, jwt_type: &JwtType) -> Result<Self> {
        // Validate and decode the JWT token
        let (validation, key) = match jwt_type {
            JwtType::None => {
                return Err(AppError::NoJwtTypeConfigured);
            }
            JwtType::HS256(key) => (
                jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
                key.clone(),
            ),
            JwtType::RS256(key) => (
                jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256),
                key.clone(),
            ),
        };
        let claims = jsonwebtoken::decode::<Claims>(
            &oauth_token.access_token().secret(),
            &key,
            &validation,
        )?;
        let result = LoginInfo {
            oauth_token,
            claims: claims.claims,
        };
        Ok(result)
    }

    pub fn api_token(&self) -> &str {
        self.oauth_token.access_token().secret()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub preferred_username: String,
    #[serde(
        default,
        rename = "https://corpus-tools.org/annis/groups",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub groups: Vec<String>,
    #[serde(
        default,
        rename = "https://corpus-tools.org/annis/roles",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub roles: Vec<String>,
}
