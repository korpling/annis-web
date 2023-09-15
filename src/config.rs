use crate::Result;
use clap::Parser;
use oauth2::{basic::BasicClient, AuthUrl, ClientId, RedirectUrl, TokenUrl};
use std::{ffi::OsString, path::PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliConfig {
    /// Port to listen to.
    #[arg(long, short, default_value_t = 3000)]
    pub port: u16,
    /// Externally used URL for the start page of the frontend.
    #[arg(long, default_value = "http://127.0.0.1:3000/")]
    pub frontend_prefix: String,
    /// URL for the graphANNIS service used by the frontend.
    #[arg(long, default_value = "http://127.0.0.1:5711/v1/")]
    pub service_url: String,

    /// If set, the SQLite database file to store sessions in.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
    /// Client name of this service when connecting to an OAuth 2.0 authorization server.
    #[arg(long, env = "ANNIS_OAUTH2_CLIENT_ID", default_value = "annis")]
    pub oauth2_client_id: String,
    /// URL of the OAuth 2.0 authorization server's authorization endpoint.
    #[arg(long, env = "ANNIS_OAUTH2_AUTH_URL")]
    pub oauth2_auth_url: Option<String>,
    /// URL of the OAuth 2.0 authorization server's token endpoint.
    #[arg(long, env = "ANNIS_OAUTH2_TOKEN_URL")]
    pub oauth2_token_url: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        let empty_arguments: Vec<OsString> = Vec::default();
        Parser::parse_from(empty_arguments)
    }
}

impl CliConfig {
    pub fn create_oauth2_basic_client(&self) -> Result<Option<BasicClient>> {
        if let (Some(auth_url), Some(token_url)) = (&self.oauth2_auth_url, &self.oauth2_token_url) {
            let redirect_url = format!("{}/oauth/callback", self.frontend_prefix);
            let client = BasicClient::new(
                ClientId::new("annis".to_string()),
                None,
                AuthUrl::new(auth_url.clone())?,
                Some(TokenUrl::new(token_url.clone())?),
            )
            .set_redirect_uri(RedirectUrl::new(redirect_url)?);
            Ok(Some(client))
        } else {
            Ok(None)
        }
    }
}
