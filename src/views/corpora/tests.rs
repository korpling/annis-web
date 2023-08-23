use crate::{
    config::CliConfig,
    tests::{get_html, start_end2end_servers},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use fantoccini::Locator;
use mockito::Server;
use scraper::Selector;
use std::{net::SocketAddr, time::Duration};
use test_log::test;
use tower::ServiceExt;

#[test(tokio::test)]
async fn list_corpora() {
    let mut env = start_end2end_servers().await;
    let m = env
        .backend
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["TueBa-D/Z.6.0", "pcc2", "pcc11", "AnyPcCorpus", "demo.dialog"]"#)
        .create();
    {
        env.webdriver
            .goto(&format!("{}/corpora", &env.frontend_addr))
            .await
            .unwrap();

        env.webdriver
            .wait()
            .for_element(Locator::XPath("//*[@id='corpus-selector']//table"))
            .await
            .unwrap();

        // The corpus list should be sorted
        let table = env
            .webdriver
            .find(Locator::XPath("//*[@id='corpus-selector']//table"))
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
    env.close().await;
}

#[test(tokio::test)]
async fn filter_corpus_name() {
    let mut env = start_end2end_servers().await;
    let _m = env
        .backend
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["TueBa-D/Z.6.0", "pcc2", "pcc11", "AnyPcCorpus", "demo.dialog"]"#)
        .create();

    env.webdriver
        .goto(&format!("{}/corpora", &env.frontend_addr))
        .await
        .unwrap();
    let input = env
        .webdriver
        .find(Locator::XPath("//*[@id='corpus-selector']//input"))
        .await
        .unwrap();
    input.send_keys("pcc").await.unwrap();

    // Wait until the table only has only three body rows
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='corpus-selector']//table/tbody[count(tr) = 3]",
        ))
        .await
        .unwrap();

    // The input must still has the focus
    let active_element = env.webdriver.active_element().await.unwrap();
    assert_eq!(
        "corpus-filter",
        active_element.attr("id").await.unwrap().unwrap_or_default()
    );

    // The corpus list should be reducted to the matching corpus names
    let table = env
        .webdriver
        .find(Locator::XPath("//*[@id='corpus-selector']//table"))
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

    env.close().await;
}

#[test(tokio::test)]
async fn add_all_filtered_corpora() {
    let mut env = start_end2end_servers().await;
    let _m = env
        .backend
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["TueBa-D/Z.6.0", "pcc2", "pcc11", "AnyPcCorpus", "demo.dialog"]"#)
        .create();

    env.webdriver
        .goto(&format!("{}/corpora", &env.frontend_addr))
        .await
        .unwrap();
    let filter_input = env
        .webdriver
        .find(Locator::XPath("//*[@id='corpus-selector']//input"))
        .await
        .unwrap();
    filter_input.send_keys("pcc").await.unwrap();

    // Wait until the table only has only three body rows
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='corpus-selector']//table/tbody[count(tr) = 3]",
        ))
        .await
        .unwrap();
    // Add all filtered corpora to the selection
    let add_all_button = env
        .webdriver
        .find(Locator::XPath(
            "//*[@id='corpus-selector']//table//button[@name='add_all_corpora']",
        ))
        .await
        .unwrap();
    add_all_button.click().await.unwrap();

    // The 3 corpora should be added to the selection
    let selected_counter = Locator::Css("#annis-navbar >* span.tag");
    assert_eq!(
        "3",
        env.webdriver
            .find(selected_counter)
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    );
    let tag_selector = Locator::Css("#corpus-selector >* span.tag");
    let tags = env.webdriver.find_all(tag_selector).await.unwrap();
    assert_eq!(3, tags.len());
    assert_eq!("AnyPcCorpus", tags[0].text().await.unwrap());
    assert_eq!("pcc11", tags[1].text().await.unwrap());
    assert_eq!("pcc2", tags[2].text().await.unwrap());

    // Clear corpora and check none is selected
    let clear_all_button = env
        .webdriver
        .find(Locator::XPath(
            "//*[@id='corpus-selector']//button[@name='clear_selection']",
        ))
        .await
        .unwrap();
    clear_all_button.click().await.unwrap();
    let tags = env.webdriver.find_all(tag_selector).await.unwrap();
    assert_eq!(0, tags.len());
    assert_eq!(
        "0",
        env.webdriver
            .find(selected_counter)
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    );
    env.close().await;
}

#[test(tokio::test)]
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
            &CliConfig::default(),
        )
        .await
        .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/corpora")
                    .body(Body::empty())
                    .unwrap(),
            )
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
