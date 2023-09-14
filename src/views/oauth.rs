use crate::{
    auth::{AnnisTokenResponse, LoginInfo},
    state::{GlobalAppState, SessionState, STATE_KEY},
    Result,
};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_sessions::extractors::WritableSession;
use minijinja::context;
use oauth2::{basic::BasicClient, TokenResponse};
use oauth2::{reqwest::async_http_client, RefreshToken};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, TokenUrl,
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
use tracing::{debug, warn};

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new()
        .route("/login", get(redirect_to_login))
        .route("/callback", get(login_callback))
        .route("/logout", get(logout));

    Ok(result)
}

fn create_client(app_state: &GlobalAppState) -> Result<BasicClient> {
    let redirect_url = format!("{}/oauth/callback", app_state.frontend_prefix.to_string());
    // TODO allow configuring the Oauth2 endpoint, e.g. from a well-known URI
    let client = BasicClient::new(
        ClientId::new("annis".to_string()),
        None,
        AuthUrl::new("http://0.0.0.0:8080/realms/ANNIS/protocol/openid-connect/auth".to_string())?,
        Some(TokenUrl::new(
            "http://0.0.0.0:8080/realms/ANNIS/protocol/openid-connect/token".to_string(),
        )?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?);
    Ok(client)
}

async fn redirect_to_login(
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let client = create_client(&app_state)?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge)
        .url();

    app_state
        .auth_requests
        .insert(csrf_token.secret().to_owned(), pkce_verifier);

    Ok(Redirect::temporary(auth_url.as_str()))
}

async fn logout(
    mut session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let mut session_state = SessionState::from(&session);

    app_state.login_info.remove(session.id());
    session_state.user_name = None;
    let template = app_state.templates.get_template("oauth.html")?;

    let html = template.render(context! {session => session_state})?;

    session.insert(STATE_KEY, session_state)?;

    Ok(Html(html))
}

#[derive(Deserialize, Debug)]
struct CallBackParams {
    error: Option<String>,
    state: Option<String>,
    code: Option<String>,
}

async fn refresh_token_action(
    refresh_instant: Instant,
    refresh_token: RefreshToken,
    client: BasicClient,
    session_id: String,
    app_state: Arc<GlobalAppState>,
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
        match login_info.renew_token(new_token.clone(), &app_state.jwt_type) {
            Ok(_) => {
                // Schedule a new token refresh if the for when the new token expires
                schedule_refresh_token(
                    &new_token,
                    client,
                    &session_id,
                    token_request_time,
                    app_state.clone(),
                )
            }
            Err(e) => {
                warn!("Could not renew-token for session {session_id}: {e}");
            }
        }
    }
    Ok(())
}

fn schedule_refresh_token(
    token: &AnnisTokenResponse,
    client: BasicClient,
    session_id: &str,
    token_request_time: Instant,
    app_state: Arc<GlobalAppState>,
) {
    if let (Some(expires_in), Some(refresh_token)) =
        (token.expires_in(), token.refresh_token().cloned())
    {
        let refresh_offset = expires_in
            .checked_sub(Duration::from_secs(10))
            .unwrap_or(expires_in);
        let refresh_instant = token_request_time.checked_add(refresh_offset);
        let session_id = session_id.to_string();
        if let Some(refresh_instant) = refresh_instant {
            tokio::spawn(refresh_token_action(
                refresh_instant,
                refresh_token,
                client,
                session_id,
                app_state,
            ));
        }
    }
}

async fn login_callback(
    mut session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Query(params): Query<CallBackParams>,
) -> Result<impl IntoResponse> {
    let mut session_state = SessionState::from(&session);

    let template = app_state.templates.get_template("oauth.html")?;

    if let Some(error) = params.error {
        let html = template.render(context! {error, session => session_state})?;
        return Ok(Html(html));
    } else if let Some(state) = params.state {
        let client = create_client(&app_state)?;

        if let Some((_, pkce_verifier)) = app_state.auth_requests.remove(&state) {
            let token_request_time = Instant::now();
            let token = client
                .exchange_code(AuthorizationCode::new(params.code.unwrap_or_default()))
                .set_pkce_verifier(pkce_verifier)
                .request_async(async_http_client)
                .await?;

            let login_info = LoginInfo::new(token.clone(), &app_state.jwt_type, &session)?;
            session_state.user_name = Some(login_info.claims.preferred_username.clone());

            app_state
                .login_info
                .insert(session.id().to_string(), login_info);

            let html = template.render(context! {
                session => session_state,
            })?;

            session.insert(STATE_KEY, session_state)?;

            // Schedule a task that refreshes the token before it expires
            schedule_refresh_token(
                &token,
                client,
                session.id(),
                token_request_time,
                app_state.clone(),
            );

            return Ok(Html(html));
        }
    }
    let html = template.render(context! {session => session_state})?;
    Ok(Html(html))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use hyper::{Body, Request, StatusCode};
    use test_log::test;
    use tower::ServiceExt;

    use crate::{config::CliConfig, tests::get_body};

    #[test(tokio::test)]
    async fn about_page_shown() {
        let app = crate::app(
            &SocketAddr::from(([127, 0, 0, 1], 3000)),
            None,
            &CliConfig::default(),
        )
        .await
        .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = get_body(response).await;
        assert!(body.contains("Not implemented yet"));
    }
}
