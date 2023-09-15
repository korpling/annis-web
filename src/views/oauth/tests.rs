use std::collections::HashMap;

use hyper::{Body, Request, StatusCode};
use scraper::Selector;
use test_log::test;
use tower::ServiceExt;
use url::Url;

use crate::{config::CliConfig, tests::get_html};

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
