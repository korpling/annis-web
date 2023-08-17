use fantoccini::Locator;
use pretty_assertions::{assert_eq, assert_ne};
use test_log::test;

use crate::{tests::start_end2end_servers, views::export::DEFAULT_EXAMPLE};

#[test(tokio::test)]
async fn export_article_noun() {
    let mut env = start_end2end_servers().await;

    // Select a corpus
    let _corpus_mock = env
        .backend
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["pcc2"]"#)
        .create();
    env.webdriver.goto(&env.frontend_addr).await.unwrap();
    env.webdriver
        .find(Locator::XPath("//button[@value='pcc2']"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // Always return the same find result
    let find_mock = env
        .backend
        .mock("POST", "/search/find")
        .with_header("content-type", "text/plain")
        .with_body(
            r#"tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
"#,
        )
        .create();

    let subgraph_mock = env
        .backend
        .mock("POST", "/corpora/pcc2/subgraph")
        .with_body_from_file("tests/export-subgraph.graphml")
        .expect_at_least(3)
        .create();

    // Switch to the export page and check that there is an initial example output
    env.webdriver
        .goto(&format!("{}/export", &env.frontend_addr))
        .await
        .unwrap();

    let initial_example_output = env
        .webdriver
        .find(Locator::XPath("//*[@id='export-example-output']/pre"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(DEFAULT_EXAMPLE, initial_example_output);

    // Change the query
    let textarea = env
        .webdriver
        .find(Locator::XPath("//textarea"))
        .await
        .unwrap();
    textarea.click().await.unwrap();
    textarea
        .send_keys("pos=\"ART\" . pos=\"NN\"")
        .await
        .unwrap();

    // Wait for the updated example output
    let updated_example_locator =
        Locator::XPath("//*[@id='export-example-output']/pre[contains(text(), 'tok_73')]");
    env.webdriver
        .wait()
        .for_element(updated_example_locator)
        .await
        .unwrap();

    find_mock.assert();
    subgraph_mock.assert();

    // Get the updated example output
    let updated_example_output = env
        .webdriver
        .find(updated_example_locator)
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_ne!(initial_example_output, updated_example_output);

    env.close().await;
}
