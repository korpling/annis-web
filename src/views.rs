use std::sync::Arc;

use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html, response::IntoResponse};

use crate::{state::GlobalAppState, Result};

#[derive(Template)]
#[template(path = "corpora.html")]
struct CorporaViewTemplate {
    corpus_names: Vec<String>,
    url_prefix: String,
}

pub async fn corpora(State(state): State<Arc<GlobalAppState>>) -> Result<impl IntoResponse> {
    let mut corpora: Vec<String> = reqwest::get(state.service_url.join("corpora")?)
        .await?
        .json()
        .await?;
    corpora.sort_unstable_by_key(|k| k.to_lowercase());

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
    use mockito::mock;
    use scraper::Selector;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn list_corpora() {
        let m = mock("GET", "/corpora")
            .with_header("content-type", "application/json")
            .with_body(r#"["pcc2", "demo.dialog"]"#)
            .create();
        {
            let app = crate::app().unwrap();

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
        m.assert();
    }
}
