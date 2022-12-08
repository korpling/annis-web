use super::*;
use crate::tests::get_html;
use axum::{body::Body, http::Request};
use mockito::mock;
use scraper::Selector;
use tower::ServiceExt;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
async fn list_corpora() {
    let m = mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["pcc2", "demo.dialog"]"#)
        .create();
    {
        let app = crate::app().unwrap();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let html = get_html(response).await;
        let list_selector = Selector::parse(".content > ul > li").unwrap();
        let corpora: Vec<_> = html
            .select(&list_selector)
            .map(|e| e.text().collect::<Vec<_>>().join(""))
            .collect();

        assert_eq!(vec!["demo.dialog", "pcc2"], corpora);
    }
    m.assert();
}

#[tokio::test]
#[traced_test]
async fn service_down() {
    // Simulate an error with the backend service
    let m = mock("GET", "/corpora").with_status(500).create();
    {
        let app = crate::app().unwrap();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // There should be an error, that the backend service access failed
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        let html = get_html(response).await;
        let list_selector = Selector::parse(".content > ul > li").unwrap();
        let corpora: Vec<_> = html
            .select(&list_selector)
            .map(|e| e.text().collect::<Vec<_>>().join(""))
            .collect();

        assert_eq!(vec!["demo.dialog", "pcc2"], corpora);
    }
    m.assert();
}
