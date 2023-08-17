use askama::Template;
use axum::{
    http::{header::InvalidHeaderValue, StatusCode},
    response::{Html, IntoResponse},
};
use reqwest::Url;
use thiserror::Error;
use tokio::task::JoinError;

use crate::state::SessionState;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    #[error(transparent)]
    Askama(#[from] askama::Error),
    #[error(transparent)]
    Axum(#[from] axum::http::Error),
    #[error(transparent)]
    AxumSerdeJson(#[from] axum_sessions::async_session::serde_json::Error),
    #[error("Got status code '{status_code}' when fetching URL '{url}' from backend.")]
    Backend { status_code: StatusCode, url: Url },
    #[error(transparent)]
    CSV(#[from] csv::Error),
    #[error(transparent)]
    GraphAnnisCore(#[from] graphannis_core::errors::GraphAnnisCoreError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    TransientBtreeIndex(#[from] transient_btree_index::Error),
    #[error(transparent)]
    UrlParsing(#[from] url::ParseError),
    #[error(transparent)]
    Sqlx(#[from] sqlx_core::error::Error),
    #[error(transparent)]
    ProgressSend(#[from] tokio::sync::mpsc::error::SendError<f32>),
    #[error(transparent)]
    JoinError(#[from] JoinError),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Download file not found.")]
    DownloadFileNotFound,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
    status_code: StatusCode,
    url_prefix: String,
    state: SessionState,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("{}", &self);
        let (status, message) = match self {
            AppError::Reqwest(e) => (StatusCode::BAD_GATEWAY, format!("{}", e)),
            AppError::DownloadFileNotFound => (StatusCode::NOT_FOUND, format!("{}", &self)),
            AppError::Backend { .. } => (StatusCode::BAD_GATEWAY, format!("{}", &self)),
            AppError::UrlParsing(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Url could not be parsed: {}", e),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", &self)),
        };
        let template = ErrorTemplate {
            message,
            status_code: status,
            url_prefix: "/".to_string(),
            state: SessionState::default(),
        };
        let html = template
            .render()
            .unwrap_or_else(|e| format!("Error page template did not compile: {}", e));
        (status, Html(html)).into_response()
    }
}
