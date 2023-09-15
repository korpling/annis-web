use std::{collections::HashMap, sync::Arc};

use axum_sessions::async_session::{MemoryStore, Session, SessionStore};
use cookie::{Cookie, CookieJar, Key};
use hyper::{Body, Request, StatusCode};
use mockito::Server;
use oauth2::{
    basic::BasicTokenType, AccessToken, PkceCodeChallenge, PkceCodeVerifier, StandardTokenResponse,
};
use scraper::Selector;
use test_log::test;
use tower::ServiceExt;
use url::Url;

use crate::{
    auth::LoginInfo,
    config::CliConfig,
    state::GlobalAppState,
    tests::{get_body, get_html},
    FALLBACK_COOKIE_KEY,
};

#[test(tokio::test)]
async fn login_rediction() {
    let mut config = CliConfig::default();
    config.oauth2_auth_url = Some("http://localhost:8080/auth".to_string());
    config.oauth2_token_url = Some("http://localhost:8080/token".to_string());

    let app = crate::app(&config).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/oauth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_redirection());
    let location = response
        .headers()
        .get("Location")
        .unwrap()
        .to_str()
        .unwrap();
    // Extract the components of the URL that should not change
    let location = Url::parse(location).unwrap();
    assert_eq!(location.host_str().unwrap(), "localhost",);
    assert_eq!(location.path(), "/auth");
    assert_eq!(location.port().unwrap(), 8080);
    let query_params: HashMap<String, String> = location
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert_eq!(query_params.len(), 6);
    assert_eq!(query_params.get("client_id").unwrap(), "annis");
    assert_eq!(
        query_params.get("redirect_uri").unwrap(),
        "http://127.0.0.1:3000//oauth/callback"
    );
    assert_eq!(query_params.get("response_type").unwrap(), "code");
    assert!(query_params.contains_key("code_challenge"));
    assert!(query_params.contains_key("code_challenge_method"));
    assert!(query_params.contains_key("state"));
}

async fn create_dummy_session() -> (String, String, MemoryStore) {
    let session = Session::new();
    let session_id = session.id().to_string();

    let session_store = MemoryStore::new();
    let unsigned_cookie_value = session_store.store_session(session).await.unwrap().unwrap();

    // Create a session cookie, which needs to be signed with the app key
    let mut cookie_jar = CookieJar::new();
    let mut session_cookie = Cookie::named("sid");
    session_cookie.set_value(unsigned_cookie_value);

    cookie_jar
        .signed_mut(&Key::from(FALLBACK_COOKIE_KEY))
        .add_original(session_cookie);
    (
        session_id,
        cookie_jar.get("sid").unwrap().to_string(),
        session_store,
    )
}

#[test(tokio::test)]
async fn logout_removes_login_info() {
    let mut config = CliConfig::default();
    config.oauth2_auth_url = Some("http://localhost:8080/auth".to_string());
    config.oauth2_token_url = Some("http://localhost:8080/token".to_string());

    // Simulate a session with a token by adding it to the session manually
    let access_token = AccessToken::new("ABC".into());
    let token_response = StandardTokenResponse::new(
        access_token,
        BasicTokenType::Bearer,
        oauth2::EmptyExtraTokenFields {},
    );

    // Create a session cookie, which needs to be signed with the app key
    let (session_id, session_cookie, session_store) = create_dummy_session().await;

    let state = Arc::new(GlobalAppState::new(&config).unwrap());
    let l = LoginInfo::new(token_response, None).unwrap();
    state.login_info.insert(session_id.clone(), l);

    // Create an app with the prepared session store
    let app = crate::app_with_state(state.clone(), session_store)
        .await
        .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/oauth/logout")
                .header("Cookie", session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Check the response
    assert!(response.status().is_success());
    let body = get_body(response).await;

    insta::assert_snapshot!("logout_removes_login_info", body);

    // The login info must be removed from the state
    assert_eq!(state.login_info.contains_key(&session_id), false);
}

#[test(tokio::test)]
async fn callback_sets_login_info() {
    // Create a mock auth server, that always returns a JWT token when requested
    let test_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0dXNlciJ9.Ad4I83jq6MsDlwFU87uVx_PaIVcmyQkV40PSI7gBJVU";
    let mut oauth_server = Server::new();
    let mut mock_token_response: HashMap<&str, serde_json::Value> = HashMap::new();
    mock_token_response.insert("access_token", test_token.into());
    mock_token_response.insert("token_type", "Bearer".into());
    mock_token_response.insert("expires_in", 36000.into());
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

    // Create a session cookie, which needs to be signed with the app key
    let (session_id, session_cookie, session_store) = create_dummy_session().await;

    // Simulate that we already started an auth request
    let app_state = Arc::new(GlobalAppState::new(&config).unwrap());
    let pkce_code = "53fa4231-2902-4f98-85f7-aebe91dfdc53.fca60b04-0ad4-497a-bf19-b0b05cda5a78.a9241b37-638b-450f-8fa4-f97f9b8fb83d";
    let state_id = "N7eDSsUS3FYBUxDAKm_jsQ";
    let pkce_verifier = PkceCodeVerifier::new(pkce_code.into());
    app_state
        .auth_requests
        .insert(state_id.to_string(), pkce_verifier);

    // Create an app with the prepared session store
    let app = crate::app_with_state(app_state.clone(), session_store)
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/oauth/callback?state={state_id}&session_state=fca60b04-0ad4-497a-bf19-b0b05cda5a78&code={pkce_code}"))
                .header("Cookie", session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The request should have triggered a token request
    mock_token_request.assert();

    // The authentification requests has to be removed from the state
    assert_eq!(app_state.auth_requests.contains_key(state_id), false);

    // A login info has been set for the current session
    let login_info = app_state.login_info.get(&session_id).unwrap();
    assert_eq!(login_info.user_id().unwrap().unwrap(), "testuser");

    // Check the response page
    let body = get_body(response).await;
    insta::assert_snapshot!("callback_sets_login_info", body);
}

#[test(tokio::test)]
async fn show_callback_error() {
    let mut config = CliConfig::default();
    config.oauth2_auth_url = Some("http://localhost:8080/auth".to_string());
    config.oauth2_token_url = Some("http://localhost:8080/token".to_string());

    // Create a session cookie, which needs to be signed with the app key
    let (session_id, session_cookie, session_store) = create_dummy_session().await;

    // Simulate that we already started an auth request
    let state = Arc::new(GlobalAppState::new(&config).unwrap());
    let (_pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    state
        .auth_requests
        .insert(session_id.clone(), pkce_verifier);

    // Create an app with the prepared session store
    let app = crate::app_with_state(state.clone(), session_store)
        .await
        .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/oauth/callback?error=this%20is%20an%20error")
                .header("Cookie", session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Check the response
    assert!(response.status().is_success());
    let body = get_body(response).await;
    insta::assert_snapshot!("show_callback_error", body);

    // The authentification requests has to be removed from the state
    assert_eq!(state.auth_requests.contains_key(&session_id), false);
}

#[test(tokio::test)]
async fn non_configured_deactivates_login() {
    let app = crate::app(&CliConfig::default()).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/about")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = get_html(response).await;
    let login_button: Vec<_> = body
        .select(&Selector::parse("div.navbar-end div.navbar-item div.buttons a.button").unwrap())
        .collect();
    assert_eq!(0, login_button.len());
}

#[test(tokio::test)]
async fn login_button_shown() {
    let mut config = CliConfig::default();
    config.oauth2_auth_url = Some("http://localhost:8080/auth".to_string());
    config.oauth2_token_url = Some("http://localhost:8080/token".to_string());
    let app = crate::app(&config).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/about")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = get_html(response).await;
    let login_button: Vec<_> = body
        .select(&Selector::parse("div.navbar-end div.navbar-item div.buttons a.button").unwrap())
        .collect();
    assert_eq!(1, login_button.len());
    assert_eq!("Log in", login_button[0].inner_html());
}
