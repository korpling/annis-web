mod components;
pub mod errors;
mod views;

use axum::{
    body::{self, Empty, Full},
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use include_dir::{include_dir, Dir};
use std::net::SocketAddr;

static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");

type Result<T> = std::result::Result<T, errors::AppError>;

async fn static_file(Path(path): Path<String>) -> Result<impl IntoResponse> {
    let path = path.trim_start_matches('/');
    let mime_type = mime_guess::from_path(path).first_or_text_plain();

    let response = match STATIC_DIR.get_file(path) {
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(body::boxed(Empty::new()))?,
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime_type.as_ref()).unwrap(),
            )
            .body(body::boxed(Full::from(file.contents())))?,
    };
    Ok(response)
}

fn app() -> Router {
    Router::new()
        .route("/", get(views::corpora))
        .route("/static/*path", get(static_file))
}

#[tokio::main]
async fn main() {
    let app = app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
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
        let app = crate::app();

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
        let app = crate::app();

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
}
