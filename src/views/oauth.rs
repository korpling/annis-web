use crate::{
    auth::{AnnisTokenResponse, LoginInfo},
    errors::AppError,
    state::{GlobalAppState, SessionState},
    Result,
};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_sessions::extractors::ReadableSession;
use minijinja::context;
use oauth2::{basic::BasicClient, TokenResponse};
use oauth2::{reqwest::async_http_client, RefreshToken};
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge};
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

async fn redirect_to_login(
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let client = app_state
        .oauth2_client
        .as_ref()
        .ok_or(AppError::Oauth2ServerConfigMissing)?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    dbg!(&pkce_challenge);
    dbg!(&pkce_verifier);

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
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state = SessionState::from(&session);

    app_state.login_info.remove(session.id());
    let template = app_state.templates.get_template("oauth.html")?;

    let html = template.render(context! {session => session_state})?;

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
        match login_info.renew_token(new_token.clone()) {
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
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Query(params): Query<CallBackParams>,
) -> Result<impl IntoResponse> {
    let session_state = SessionState::from(&session);

    let template = app_state.templates.get_template("oauth.html")?;

    if let Some(error) = params.error {
        let html = template.render(context! {error, session => session_state})?;

        if let Some(state) = params.state {
            app_state.auth_requests.remove(&state);
        }

        return Ok(Html(html));
    } else if let Some(state) = params.state {
        let client = app_state
            .oauth2_client
            .as_ref()
            .ok_or(AppError::Oauth2ServerConfigMissing)?;

        if let Some((_, pkce_verifier)) = app_state.auth_requests.remove(&state) {
            let token_request_time = Instant::now();
            let token = client
                .exchange_code(AuthorizationCode::new(params.code.unwrap_or_default()))
                .set_pkce_verifier(pkce_verifier)
                .request_async(async_http_client)
                .await;
            dbg!(&token);
            let token = token?;

            let login_info = LoginInfo::new(token.clone(), session.expiry().cloned())?;

            app_state
                .login_info
                .insert(session.id().to_string(), login_info);

            let html = template.render(context! {
                session => session_state,
            })?;

            // Schedule a task that refreshes the token before it expires
            schedule_refresh_token(
                &token,
                client.clone(),
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
mod tests;
