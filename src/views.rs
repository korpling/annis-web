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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::get_html;
    use axum::{body::Body, http::Request};
    use scraper::Selector;
    use tower::ServiceExt;

    #[tokio::test]
    async fn list_corpora() {
        let app = crate::app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let html = get_html(response).await;
        let list_selector = Selector::parse(".content > ul > li").unwrap();
        let corpora: Vec<_> = html
            .select(&list_selector)
            .map(|e| e.text().collect::<Vec<_>>().join(""))
            .collect();

        assert_eq!(vec!["pcc2", "demo.dialog"], corpora);
    }
}
