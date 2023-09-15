use std::{collections::HashMap, sync::Arc};

use axum_sessions::async_session::{MemoryStore, Session, SessionStore};
use cookie::{Cookie, CookieJar, Key};
use hyper::{Body, Request, StatusCode};
use oauth2::{basic::BasicTokenType, AccessToken, StandardTokenResponse};
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

    let session = Session::new();
    let session_id = session.id().to_string();
    let l = LoginInfo::new(token_response, &session).unwrap();

    let state = Arc::new(GlobalAppState::new(&config).unwrap());
    state.login_info.insert(session_id.clone(), l);

    let session_store = MemoryStore::new();
    session_store.store_session(session.clone()).await.unwrap();

    // Create an app with the prepared session store
    let app = crate::app_with_state(state.clone(), session_store)
        .await
        .unwrap();

    // Create a session cookie, which needs to be signed with the app key
    let mut cookie_jar = CookieJar::new();
    let mut session_cookie = Cookie::named("sid");
    session_cookie.set_value(session.into_cookie_value().unwrap());

    cookie_jar
        .signed_mut(&Key::from(FALLBACK_COOKIE_KEY))
        .add_original(session_cookie);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/oauth/logout")
                .header("Cookie", cookie_jar.get("sid").unwrap().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Check the response
    assert!(response.status().is_success());
    let body = get_body(response).await;
    insta::assert_snapshot!(body);

    // The login info must be removed from the state
    assert_eq!(state.login_info.contains_key(&session_id), false);
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
