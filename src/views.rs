use std::vec;

use askama::Template;
use axum::{http::StatusCode, response::Html, response::IntoResponse};

#[derive(Template)]
#[template(path = "corpora.html")]
struct CorporaViewTemplate {
    corpus_names: Vec<String>,
    url_prefix: String,
}

pub async fn corpora() -> impl IntoResponse {
    let template = CorporaViewTemplate {
        url_prefix: "/".to_string(),
        corpus_names: vec!["pcc2".to_string(), "demo.dialog".to_string()],
    };
    (StatusCode::OK, Html(template.render().unwrap()))
}
