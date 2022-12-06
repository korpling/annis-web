use askama::Template;
use axum::{http::StatusCode, response::Html, response::IntoResponse};

use crate::Result;

#[derive(Template)]
#[template(path = "corpora.html")]
struct CorporaViewTemplate {
    corpus_names: Vec<String>,
    url_prefix: String,
}

pub async fn corpora() -> Result<impl IntoResponse> {
    let mut corpora: Vec<String> = vec!["pcc2", "dialog.demo"];
    corpora.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let template = CorporaViewTemplate {
        url_prefix: "/".to_string(),
        corpus_names: corpora,
    };
    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
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

        assert_eq!(vec!["demo.dialog", "pcc2"], corpora);
    }
}
