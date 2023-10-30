use crate::{
    state::{GlobalAppState, Session},
    Result,
};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use minijinja::context;
use std::sync::Arc;

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new().route("/", get(show));
    Ok(result)
}

async fn show(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let html = app_state
        .templates
        .get_template("about.html")?
        .render(context! {
            session => session,
            version => env!("CARGO_PKG_VERSION"),
        })?;

    Ok(Html(html))
}

#[cfg(test)]
mod tests {

    use hyper::{Body, Request, StatusCode};
    use test_log::test;
    use tower::ServiceExt;

    use crate::{tests::get_body, CliConfig};

    #[test(tokio::test)]
    async fn about_page_shown() {
        let app = crate::app(&CliConfig::default()).await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/about")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = get_body(response).await;
        assert!(body.contains("Version"));
    }
}
