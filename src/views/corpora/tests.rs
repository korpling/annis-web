use super::*;
use crate::tests::{get_html, start_end2end_servers};
use axum::{body::Body, http::Request};
use fantoccini::Locator;
use mockito::Server;
use scraper::Selector;
use std::{net::SocketAddr, thread, time::Duration};
use tower::ServiceExt;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
async fn list_corpora() {
    let (c, mut service_mock, url) = start_end2end_servers().await;
    let m = service_mock
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["TueBa-D/Z.6.0", "pcc2", "pcc11", "AnyPcCorpus", "demo.dialog"]"#)
        .create();
    {
        c.goto(&url).await.unwrap();

        c.wait()
            .for_element(Locator::XPath(
                "//*[@id='corpus-selector']/div/div[2]/article/div[2]/table",
            ))
            .await
            .unwrap();

        // The corpus list should be sorted
        let table = c
            .find(Locator::XPath(
                "//*[@id='corpus-selector']/div/div[2]/article/div[2]/table",
            ))
            .await
            .unwrap();

        let rows = table.find_all(Locator::Css("tbody tr")).await.unwrap();
        assert_eq!(5, rows.len());
        assert_eq!(
            "AnyPcCorpus",
            rows[0]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "demo.dialog",
            rows[1]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "pcc11",
            rows[2]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "pcc2",
            rows[3]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "TueBa-D/Z.6.0",
            rows[4]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
    }

    m.assert();
    c.close().await.unwrap();
}

#[tokio::test]
#[traced_test]
async fn filter_corpus_name() {
    let (c, mut service_mock, url) = start_end2end_servers().await;
    let _m = service_mock
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["TueBa-D/Z.6.0", "pcc2", "pcc11", "AnyPcCorpus", "demo.dialog"]"#)
        .create();
    {
        c.goto(&url).await.unwrap();
        let input = c
            .find(Locator::XPath(
                "//*[@id='corpus-selector']/div/div[2]/article/div[1]/div[1]/input",
            ))
            .await
            .unwrap();
        input.send_keys("pcc").await.unwrap();

        thread::sleep(Duration::from_secs(5));

        // The input must still has the focus
        let active_element = c.active_element().await.unwrap();
        assert_eq!(input.element_id(), active_element.element_id());

        // The corpus list should be reducted to the matching corpus names
        let table = c
            .find(Locator::XPath(
                "//*[@id='corpus-selector']/div/div[2]/article/div[2]/table",
            ))
            .await
            .unwrap();

        let rows = table.find_all(Locator::Css("tbody tr")).await.unwrap();
        assert_eq!(3, rows.len());
        assert_eq!(
            "AnyPcCorpus",
            rows[0]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "pcc11",
            rows[1]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
        assert_eq!(
            "pcc2",
            rows[2]
                .find(Locator::Css("td.corpus-name"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
    }
    c.close().await.unwrap();
}

#[tokio::test]
#[traced_test]
async fn service_down() {
    // Simulate an error with the backend service
    let mut service_mock = Server::new_with_port(0);
    let m = service_mock
        .mock("GET", "/corpora")
        .with_status(500)
        .create();
    {
        let app = crate::app(
            &SocketAddr::from(([127, 0, 0, 1], 3000)),
            Some(&service_mock.url()),
            None,
        )
        .await
        .unwrap();

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
