use std::fmt::Display;

use axum::{
    http::{header::InvalidHeaderValue, StatusCode},
    response::{Html, IntoResponse},
};
use minijinja::context;
use reqwest::Url;
use serde::Deserialize;
use thiserror::Error;
use tokio::task::JoinError;

use crate::state::SessionState;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for LineColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct LineColumnRange {
    pub start: LineColumn,
    pub end: Option<LineColumn>,
}

impl std::fmt::Display for LineColumnRange {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(end) = self.end.clone() {
            if self.start == end {
                write!(f, "{}", self.start)
            } else {
                write!(f, "{}-{}", self.start, end)
            }
        } else {
            write!(f, "{}", self.start)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AQLError {
    pub desc: String,
    pub location: Option<LineColumnRange>,
}

impl Display for AQLError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(location) = &self.location {
            write!(f, "[{}] {}", location, self.desc)
        } else {
            write!(f, "{}", self.desc)
        }
    }
}

#[derive(Deserialize, Debug)]
#[non_exhaustive]

pub enum BadRequestError {
    AQLSyntaxError(AQLError),
    AQLSemanticError(AQLError),
    ImpossibleSearch(String),
    Uuid(String),
    IllegalNodePath(String),
}

impl Display for BadRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BadRequestError::AQLSyntaxError(e) => write!(f, "Syntax error in query:\n{e}"),
            BadRequestError::AQLSemanticError(e) => write!(f, "Semantic error in query:\n{e}"),
            BadRequestError::ImpossibleSearch(msg) => {
                write!(f, "The given query can not give any results:\n{msg}")
            }
            BadRequestError::Uuid(msg) => write!(f, "Issue with UUID:\n{msg}"),
            BadRequestError::IllegalNodePath(msg) => write!(f, "Illegal node path used:\n{msg}"),
        }
    }
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    #[error(transparent)]
    Axum(#[from] axum::http::Error),
    #[error(transparent)]
    AxumSerdeJson(#[from] axum_sessions::async_session::serde_json::Error),
    #[error("Got status code '{status_code}' when fetching URL '{url}' from backend.")]
    Backend { status_code: StatusCode, url: Url },
    #[error("{0}")]
    BackendBadRequest(BadRequestError),
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
    #[error(transparent)]
    MiniJinja(#[from] minijinja::Error),
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
        // Render the template with the error message
        let mut env = minijinja::Environment::new();
        let result = env
            .add_template("base.html", include_str!("../templates/base.html"))
            .and_then(|_| env.template_from_str(include_str!("../templates/error.html")))
            .and_then(|template| {
                template.render(context! {
                    message,
                    status_code => status.as_u16(),
                    canonical_reason => status.canonical_reason().unwrap_or_default(),
                    // TODO: how can we find the actual prefix without having access to the session?
                    url_prefix=> "/".to_string(),
                    session => SessionState::default(),
                })
            })
            .unwrap_or_else(|e| format!("Error page template did not render: {}", e));
        (status, Html(result)).into_response()
    }
}
