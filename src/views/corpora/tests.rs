use crate::{
    config::CliConfig,
    tests::{get_body, start_end2end_servers},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use fantoccini::Locator;
use mockito::Server;
use std::time::Duration;
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
    tokio::time::sleep(Duration::from_millis(500)).await;
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
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='annis-corpora-navbar-item']/span[text()='3']",
        ))
        .await
        .unwrap();
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='corpus-selector']//div[count(span)=3]",
        ))
        .await
        .unwrap();
    let tag_selector = Locator::Css("#corpus-selector >* span.tag");
    let tags = env.webdriver.find_all(tag_selector).await.unwrap();
    assert_eq!(tags.len(), 3);
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
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='corpus-selector']//div[count(span)=0]",
        ))
        .await
        .unwrap();

    let tags = env.webdriver.find_all(tag_selector).await.unwrap();
    assert_eq!(0, tags.len());
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::XPath(
            "//*[@id='annis-corpora-navbar-item']/span[text()='0']",
        ))
        .await
        .unwrap();

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
        let mut config = CliConfig::default();
        config.service_url = service_mock.url();
        let app = crate::app(&config).await.unwrap();

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

        let body = get_body(response).await;

        insta::assert_snapshot!("service_down", body);
    }
    m.assert();
}
