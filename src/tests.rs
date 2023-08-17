use std::{
    fmt::Debug,
    net::{SocketAddr, TcpListener},
};

use axum::{
    body::HttpBody,
    http::{Request, Response},
};
use fantoccini::{wd::Capabilities, ClientBuilder};
use hyper::{Body, StatusCode};
use scraper::Html;
use serde_json::json;
use tempfile::TempDir;
use test_log::test;
use tokio::task::JoinHandle;
use tower::ServiceExt;

pub struct TestEnvironment {
    pub webdriver: fantoccini::Client,
    pub backend: mockito::Server,
    pub frontend: JoinHandle<()>,
    pub frontend_addr: String,
    pub download_folder: TempDir,
}

impl TestEnvironment {
    pub async fn close(self) {
        self.webdriver.close().await.unwrap();
        self.frontend.abort();
        self.download_folder.close().unwrap();
    }
}

pub async fn start_end2end_servers() -> TestEnvironment {
    let service_mock = mockito::Server::new_with_port(0);

    // Create a temporary folder used for downloaded files. In case the browser
    // is restricted to only to be allowed to operate in the download folder of
    // the user, use a temporary subdirectory inside the download folder.
    let download_folder = if let Some(user_download) = dirs::download_dir() {
        TempDir::new_in(user_download)
    } else {
        TempDir::new()
    }
    .unwrap();

    // Configure the browser to autoamtically accept downloads and add them the given folder
    let mut browser_capabilities = Capabilities::default();
    browser_capabilities.insert(
        "goog:chromeOptions".to_string(),
        json!({
                "prefs": {
                    "download": {
                        "default_directory": download_folder.path().to_string_lossy(),
                    },
                }
            }
        ),
    );

    let webdriver = ClientBuilder::native()
        .capabilities(browser_capabilities)
        .connect("http://localhost:4444")
        .await
        .expect("failed to connect to WebDriver");

    let listener = TcpListener::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    let service_mock_url = service_mock.url();

    let http_server = tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(
                crate::app(&addr, Some(&service_mock_url), None)
                    .await
                    .unwrap()
                    .into_make_service(),
            )
            .await
            .unwrap();
    });
    TestEnvironment {
        webdriver,
        backend: service_mock,
        frontend: http_server,
        frontend_addr: format!("http://{}", addr),
        download_folder,
    }
}

pub async fn get_body<T>(response: Response<T>) -> String
where
    T: HttpBody,
    <T as HttpBody>::Error: Debug,
{
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body = String::from_utf8_lossy(&body[..]);
    body.to_string()
}

pub async fn get_html<T>(response: Response<T>) -> Html
where
    T: HttpBody,
    <T as HttpBody>::Error: Debug,
{
    let body = get_body(response).await;
    Html::parse_document(&body)
}

#[test(tokio::test)]
async fn existing_static_resource() {
    let app = crate::app(&SocketAddr::from(([127, 0, 0, 1], 3000)), None, None)
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/static/README.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = get_body(response).await;
    assert_eq!(
        "This folder contains static ressources used by the web application.",
        &body
    );
}

#[test(tokio::test)]
async fn missing_static_resource() {
    let app = crate::app(&SocketAddr::from(([127, 0, 0, 1], 3000)), None, None)
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/static/THIS_FILE_DOES_NOT_EXIST.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
