use std::time::Duration;

use fantoccini::{Client, Locator};
use mockito::{Matcher, Mock};
use pretty_assertions::{assert_eq, assert_ne};
use test_log::test;

use crate::{
    tests::{start_end2end_servers, TestEnvironment},
    views::export::DEFAULT_EXAMPLE,
};

async fn select_corpus_and_goto_export_pcc(env: &mut TestEnvironment) {
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
    env.webdriver
        .goto(&format!("{}/export", &env.frontend_addr))
        .await
        .unwrap();
}

async fn select_corpus_and_goto_export_ridges(env: &mut TestEnvironment) {
    let _corpus_mock = env
        .backend
        .mock("GET", "/corpora")
        .with_header("content-type", "application/json")
        .with_body(r#"["RIDGES_Herbology_Version9.0"]"#)
        .create();
    env.webdriver.goto(&env.frontend_addr).await.unwrap();
    env.webdriver
        .find(Locator::XPath(
            "//button[@value='RIDGES_Herbology_Version9.0']",
        ))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    env.webdriver
        .goto(&format!("{}/export", &env.frontend_addr))
        .await
        .unwrap();
}

async fn enter_query(webdriver: &Client) {
    let textarea = webdriver.find(Locator::XPath("//textarea")).await.unwrap();
    textarea.click().await.unwrap();
    textarea
        .send_keys("pos=\"ART\" . pos=\"NN\"")
        .await
        .unwrap();
}

fn create_conversion_mocks(backend: &mut mockito::Server) -> (Mock, Mock) {
    let find_mock = backend
        .mock("POST", "/search/find")
        .with_header("content-type", "text/plain")
        .with_body(
            r#"tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
"#,
        )
        .create();

    let subgraph_mock = backend
        .mock("POST", "/corpora/pcc2/subgraph")
        .with_body_from_file("tests/export-pcc2.graphml")
        .expect_at_least(3)
        .create();

    (find_mock, subgraph_mock)
}

#[test(tokio::test)]
async fn export_preview() {
    let mut env = start_end2end_servers().await;

    select_corpus_and_goto_export_pcc(&mut env).await;

    // Switch to the export page and check that there is an initial example output
    let initial_example_output = env
        .webdriver
        .find(Locator::XPath("//*[@id='export-example-output']/pre"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(DEFAULT_EXAMPLE, initial_example_output);

    // Always return the same find result
    let (find_mock, subgraph_mock) = create_conversion_mocks(&mut env.backend);

    // Change the query
    enter_query(&env.webdriver).await;

    // Wait for the updated example output
    let updated_example_locator = Locator::XPath(
        "//*[@id='export-example-output']/pre[contains(text(), 'haben den Ball erst')]",
    );
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

#[test(tokio::test)]
async fn change_csv_export_params_pcc() {
    let mut env = start_end2end_servers().await;

    select_corpus_and_goto_export_pcc(&mut env).await;

    let _find_mock = env
        .backend
        .mock("POST", "/search/find")
        .with_header("content-type", "text/plain")
        .with_body(r#"pcc2/4282#tok_73"#)
        .create();

    let components_mock = env
        .backend
        .mock("GET", "/corpora/pcc2/components")
        .match_query(Matcher::UrlEncoded("type".into(), "Ordering".into()))
        .with_header("content-type", "text/plain")
        .with_body(
            r#"[{
            "type": "Ordering",
            "name": "",
            layer: "annis"
        }]"#,
        )
        .create();

    // Change the query parameters
    let f = env.webdriver.form(Locator::Css("form")).await.unwrap();
    f.set_by_name("left_context", "10").await.unwrap();
    f.set_by_name("right_context", "5").await.unwrap();

    let subgraph_mock = env
        .backend
        .mock("POST", "/corpora/pcc2/subgraph")
        .match_body(Matcher::PartialJsonString(
            r#"
            {
                "node_ids": ["pcc2/4282#tok_73"],
                "left": 10,
                "right" : 5
            }"#
            .into(),
        ))
        .with_body_from_file("tests/export-pcc2.graphml")
        .expect(1)
        .create();

    enter_query(&env.webdriver).await;

    // Wait for the updated example output
    let updated_example_locator = Locator::XPath(
        "//*[@id='export-example-output']/pre[contains(text(), 'haben den Ball erst')]",
    );
    env.webdriver
        .wait()
        .for_element(updated_example_locator)
        .await
        .unwrap();

    subgraph_mock.assert();
    components_mock.assert();

    env.close().await;
}

#[test(tokio::test)]
async fn change_csv_export_params_ridges() {
    let mut env = start_end2end_servers().await;

    let components_mock = env
        .backend
        .mock("GET", "/corpora/RIDGES_Herbology_Version9.0/components")
        .match_query(Matcher::UrlEncoded("type".into(), "Ordering".into()))
        .with_body(
            r#"[{
            "type": "Ordering",
            "name": "",
            "layer": "annis"
        },
        {
            "type": "Ordering",
            "name": "dipl",
            "layer": "default_ns"
        },
        {
            "type": "Ordering",
            "name": "norm",
            "layer": "default_ns"
        }]"#,
        )
        .expect_at_least(1)
        .create();
    select_corpus_and_goto_export_ridges(&mut env).await;

    components_mock.assert();

    // Change the query parameters
    let f = env.webdriver.form(Locator::Css("form")).await.unwrap();
    f.set_by_name("span_segmentation", "dipl").await.unwrap();
    f.set_by_name("left_context", "10").await.unwrap();
    f.set_by_name("right_context", "5").await.unwrap();

    let _find_mock = env
        .backend
        .mock("POST", "/search/find")
        .with_header("content-type", "text/plain")
        .with_body(
            r#"RIDGES_Herbology_Version9.0/Experimenta_1550_Schellenberg#sTok2771_virtualSpan"#,
        )
        .create();

    let subgraph_mock = env
        .backend
        .mock("POST", "/corpora/RIDGES_Herbology_Version9.0/subgraph")
        .match_body(Matcher::PartialJsonString(
            r#"
            {
                "node_ids": ["RIDGES_Herbology_Version9.0/Experimenta_1550_Schellenberg#sTok2771_virtualSpan"],
                "segmentation": "dipl",
                "left": 10,
                "right" : 5
            }"#
            .into(),
        ))
        .with_body_from_file("tests/ridges-subgraph.graphml")
        .expect_at_least(1)
        .create();

    enter_query(&env.webdriver).await;

    // Wait for the updated example output
    let updated_example_locator =
        Locator::XPath("//*[@id='export-example-output']/pre[contains(text(), 'Baldrian')]");
    env.webdriver
        .wait()
        .for_element(updated_example_locator)
        .await
        .unwrap();

    subgraph_mock.assert();

    env.close().await;
}

#[test(tokio::test)]
async fn export_cancel() {
    let mut env = start_end2end_servers().await;

    create_conversion_mocks(&mut env.backend);
    select_corpus_and_goto_export_pcc(&mut env).await;

    // Set query and start export
    enter_query(&env.webdriver).await;

    let start_button_locator = Locator::XPath("//button[contains(text(), 'Start export')]");
    let start_button = env.webdriver.find(start_button_locator).await.unwrap();
    start_button.click().await.unwrap();

    // Wait and click on the cancel button
    let cancel_button_locator = Locator::XPath("//button[contains(text(), 'Cancel')]");
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(cancel_button_locator)
        .await
        .unwrap();
    env.webdriver
        .find(cancel_button_locator)
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // The start export button should appear again
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(start_button_locator)
        .await
        .unwrap();
    env.close().await;
}

#[test(tokio::test)]
async fn export_download() {
    let mut env = start_end2end_servers().await;

    create_conversion_mocks(&mut env.backend);
    select_corpus_and_goto_export_pcc(&mut env).await;

    // Set query and start export
    enter_query(&env.webdriver).await;

    let start_button_locator = Locator::XPath("//button[contains(text(), 'Start export')]");
    let start_button = env.webdriver.find(start_button_locator).await.unwrap();
    start_button.click().await.unwrap();

    // This will trigger a conversion and automatic download, wait until the start button appears again
    env.webdriver
        .wait()
        .for_element(start_button_locator)
        .await
        .unwrap();

    // Get the downloaded file
    let expected_file_path = env.download_folder.path().join("annis-export.csv");
    let mut found_file = false;
    for _ in 0..5 {
        if expected_file_path.exists() {
            found_file = true;
            let file_content = std::fs::read_to_string(&expected_file_path).unwrap();

            assert_eq!(
                r#"text,tiger::lemma (1),tiger::morph (1),tiger::pos (1),tiger::lemma (2),tiger::morph (2),tiger::pos (2)
haben den Ball erst,der,Acc.Sg.Masc,ART,Ball,Acc.Sg.Masc,NN
haben den Ball erst,der,Acc.Sg.Masc,ART,Ball,Acc.Sg.Masc,NN
haben den Ball erst,der,Acc.Sg.Masc,ART,Ball,Acc.Sg.Masc,NN
"#,
                file_content
            );
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    assert_eq!(true, found_file);

    env.close().await;
}

#[test(tokio::test)]
async fn syntax_error() {
    let mut env = start_end2end_servers().await;

    select_corpus_and_goto_export_pcc(&mut env).await;

    let find_mock_with_error = env
        .backend
        .mock("POST", "/search/find")
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "AQLSyntaxError": {
                    "desc": "Invalid token detected.",
                    "location": {
                        "start": {
                            "line": 1,
                            "column": 5
                        },
                        "end": {
                            "line": 1,
                            "column": 5
                        }
                    }
                }
            }"#,
        )
        .with_status(400)
        .create();

    let textarea = env
        .webdriver
        .find(Locator::XPath("//textarea"))
        .await
        .unwrap();
    textarea.click().await.unwrap();
    textarea.send_keys("tok=\"").await.unwrap();

    // Wait until the error message is shown (this will take same time since there is a delay when sending the keys)
    let error_locator = Locator::Css("#export-example-output > div.is-danger");
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(error_locator)
        .await
        .unwrap();

    let error_div = env.webdriver.find(error_locator).await.unwrap();
    assert_eq!(
        error_div.text().await.unwrap(),
        "Syntax error in query: [1:5] Invalid token detected."
    );

    find_mock_with_error.expect(1).assert();

    env.close().await;
}

#[test(tokio::test)]
async fn backend_down() {
    let mut env = start_end2end_servers().await;

    select_corpus_and_goto_export_pcc(&mut env).await;

    let find_mock_with_error = env
        .backend
        .mock("POST", "/search/find")
        .with_header("content-type", "application/json")
        .with_status(502)
        .create();

    enter_query(&env.webdriver).await;

    // Wait until the error message is shown (this will take same time since there is a delay when sending the keys)
    let error_locator = Locator::Css("#export-example-output > div.is-danger");
    env.webdriver
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(error_locator)
        .await
        .unwrap();

    let error_div = env.webdriver.find(error_locator).await.unwrap();
    assert_eq!(
        error_div.text().await.unwrap(),
        format!(
            "Got status code '502 Bad Gateway' when fetching URL '{}/search/find' from backend.",
            env.backend.url()
        )
    );

    find_mock_with_error.expect(1).assert();

    env.close().await;
}
