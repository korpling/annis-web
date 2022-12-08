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
        let list_selector = Selector::parse(".box > ul > li").unwrap();
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

        // Two tiles should exist, one with the error description and an info
        // box what to do about it.
        let tile_selector = Selector::parse("article.tile.is-child").unwrap();
        let tiles: Vec<_> = html.select(&tile_selector).collect();
        assert_eq!(2, tiles.len());

        // Check the title (with proper status code) and that the error message
        // is displayed
        let subtitle_selector = Selector::parse("h2.subtitle").unwrap();
        let subtitle_error = tiles[0]
            .select(&subtitle_selector)
            .next()
            .unwrap()
            .text()
            .next()
            .unwrap()
            .trim();
        assert_eq!(
            concat!(
                "The code of the error is 502 (Bad Gateway) and the following ",
                "message describes the issue in more detail:"
            ),
            subtitle_error
        );

        let content_selector = Selector::parse(".content").unwrap();
        let error_message = tiles[0]
            .select(&content_selector)
            .next()
            .unwrap()
            .text()
            .next()
            .unwrap()
            .trim();
        assert_eq!(
            "error decoding response body: EOF while parsing a value at line 1 column 0",
            error_message
        );

        // Also check that the second tile with the helpful information is there
        let title_selector = Selector::parse("h1.title").unwrap();
        let info_subtitle = tiles[1]
            .select(&title_selector)
            .next()
            .unwrap()
            .text()
            .next()
            .unwrap()
            .trim();
        assert_eq!("What can you do?", info_subtitle);
    }
    m.assert();
}
