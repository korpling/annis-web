use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    #[error(transparent)]
    Axum(#[from] axum::http::Error),
    #[error(transparent)]
    Askama(#[from] askama::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    UrlParsing(#[from] url::ParseError),
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
    status_code: StatusCode,
    url_prefix: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        tracing::trace!("{}", &self);
        let (status, message) = match self {
            AppError::Axum(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
            AppError::Askama(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
            AppError::Reqwest(e) => (StatusCode::BAD_GATEWAY, format!("{}", e)),
            AppError::UrlParsing(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Url could not be parsed: {}", e),
            ),
        };
        let template = ErrorTemplate {
            message,
            status_code: status,
            url_prefix: "/".to_string(),
        };
        let html = template
            .render()
            .unwrap_or_else(|e| format!("Error page template did not compile: {}", e));
        (status, Html(html)).into_response()
    }
}
