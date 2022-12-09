use std::{
    fmt::Debug,
    net::{SocketAddr, TcpListener},
};

use axum::{
    body::HttpBody,
    http::{Request, Response},
};
use fantoccini::ClientBuilder;
use hyper::{Body, StatusCode};
use scraper::Html;
use tower::ServiceExt;

pub async fn start_end2end_servers() -> (fantoccini::Client, String) {
    let c = ClientBuilder::native()
        .connect("http://localhost:4444")
        .await
        .expect("failed to connect to WebDriver");

    let listener = TcpListener::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(crate::app().unwrap().into_make_service())
            .await
            .unwrap();
    });
    (c, format!("http://{}", addr))
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

#[tokio::test]
async fn existing_static_resource() {
    let app = crate::app().unwrap();

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

#[tokio::test]
async fn missing_static_resource() {
    let app = crate::app().unwrap();

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
