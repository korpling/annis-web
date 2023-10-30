use crate::{
    auth::{schedule_refresh_token, LoginInfo},
    errors::AppError,
    state::{GlobalAppState, Session},
    Result,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};

use minijinja::context;
use oauth2::reqwest::async_http_client;
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

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
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    app_state.login_info.remove(&session.id().to_string());
    let template = app_state.templates.get_template("oauth.html")?;

    let html = template.render(context! {session => session})?;

    Ok(Html(html))
}

#[derive(Deserialize, Debug)]
struct CallBackParams {
    error: Option<String>,
    state: Option<String>,
    code: Option<String>,
}

async fn login_callback(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
    Query(params): Query<CallBackParams>,
) -> Result<impl IntoResponse> {
    let template = app_state.templates.get_template("oauth.html")?;

    if let Some(error) = params.error {
        let html = template.render(context! {error, session => session})?;

        if let Some(state) = params.state {
            app_state.auth_requests.remove(&state);
        }

        return Ok((StatusCode::BAD_GATEWAY, Html(html)));
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
            let token = token?;

            let login_info = LoginInfo::new(
                token.clone(),
                session.expiration_time().map(|t| t.unix_timestamp()),
            )?;

            app_state
                .login_info
                .insert(session.id().to_string(), login_info);

            let html = template.render(context! {
                session => session,
            })?;

            // Schedule a task that refreshes the token before it expires
            schedule_refresh_token(
                &token,
                client.clone(),
                session.id(),
                token_request_time,
                app_state.clone(),
                Duration::from_secs(10),
            );

            return Ok((StatusCode::OK, Html(html)));
        }
    }
    let html =
        template.render(context! {session => session, error => "Empty authorization request."})?;
    Ok((StatusCode::BAD_REQUEST, Html(html)))
}

#[cfg(test)]
mod tests;
