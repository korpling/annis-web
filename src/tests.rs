use std::fmt::Debug;

use axum::{
    body::HttpBody,
    http::{Request, Response},
};
use hyper::{Body, StatusCode};
use scraper::Html;
use tower::ServiceExt;

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
