use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Axum(#[from] axum::http::Error),
    #[error(transparent)]
    Askama(#[from] askama::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("unknown error")]
    Unknown,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
    status: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Unknown => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unknown error".to_string(),
            ),
            AppError::Axum(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
            AppError::Askama(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
            AppError::Reqwest(e) => (StatusCode::BAD_GATEWAY, format!("{}", e)),
        };
        let template = ErrorTemplate {
            message,
            status: status.as_u16(),
        };
        let html = template
            .render()
            .unwrap_or("Error page template did not compile".to_string());
        (status, Html(html)).into_response()
    }
}
