use crate::{
    state::{GlobalAppState, SessionState},
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
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, TokenUrl,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new()
        .route("/", get(redirect_to_login))
        .route("/callback", get(login_callback));

    Ok(result)
}

fn create_client(app_state: &GlobalAppState) -> Result<BasicClient> {
    let redirect_url = format!("{}login/callback", app_state.frontend_prefix.to_string());
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

    dbg!(pkce_challenge.as_str());

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    dbg!(csrf_token.secret());

    app_state
        .auth_requests
        .insert(csrf_token.secret().to_owned(), pkce_verifier);

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[derive(Deserialize, Debug)]
struct CallBackParams {
    error: Option<String>,
    state: Option<String>,
    code: Option<String>,
}

async fn login_callback(
    session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Query(params): Query<CallBackParams>,
) -> Result<impl IntoResponse> {
    let session_state = SessionState::from(&session);

    let template = app_state.templates.get_template("login.html")?;

    if let Some(error) = params.error {
        let html = template.render(context! {error, session => session_state})?;
        return Ok(Html(html));
    } else if let Some(state) = params.state {
        let client = create_client(&app_state)?;

        if let Some((_, pkce_verifier)) = app_state.auth_requests.remove(&state) {
            dbg!(&pkce_verifier);
            let token = client
                .exchange_code(AuthorizationCode::new(params.code.unwrap_or_default()))
                // Set the PKCE code verifier.
                .set_pkce_verifier(pkce_verifier)
                .request_async(async_http_client)
                .await?;

            let html = template.render(context! {
                session => session_state, token
            })?;
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

    use crate::tests::get_body;

    #[test(tokio::test)]
    async fn about_page_shown() {
        let app = crate::app(&SocketAddr::from(([127, 0, 0, 1], 3000)), None, None)
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
