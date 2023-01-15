use std::sync::Arc;

use crate::{
    client::search::FindQuery,
    converter::CSVExporter,
    state::{GlobalAppState, SessionState},
    Result,
};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use axum_sessions::extractors::WritableSession;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use serde::Deserialize;

const DEFAULT_EXAMPLE: &str = r#"match number,1 node name,1 tiger::lemma,1 tiger::morph,1 tiger::pos
1,pcc2/11299#tok_1,Feigenblatt,Nom.Sg.Neut,NN
2,pcc2/11299#tok_2,der,Nom.Pl.*,ART
3,pcc2/11299#tok_3,jugendliche,Nom.Pl.*,NN"#;

#[derive(Template, Debug)]
#[template(path = "export-example-output.html")]
struct ExampleOutput {
    example: String,
}

impl Default for ExampleOutput {
    fn default() -> Self {
        Self {
            example: DEFAULT_EXAMPLE.to_string(),
        }
    }
}

#[derive(Template, Debug)]
#[template(path = "export.html")]
struct Export {
    url_prefix: String,
    state: SessionState,
    example_output: ExampleOutput,
}

pub async fn get(
    session: WritableSession,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let template = Export {
        url_prefix: state.frontend_prefix.to_string(),
        example_output: ExampleOutput::default(),
        state: session_state,
    };

    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}

#[derive(Deserialize, Debug)]
pub struct UpdateExampleParams {
    query: String,
}

pub async fn update_example(
    session: WritableSession,
    Query(params): Query<UpdateExampleParams>,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let example_query = FindQuery {
        query: params.query,
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };

    let mut template = ExampleOutput::default();

    let mut exporter = CSVExporter::new(example_query);
    let mut example_string_buffer = Vec::new();
    exporter
        .convert_text(state.as_ref(), Some(3), &mut example_string_buffer)
        .await?;

    template.example = String::from_utf8_lossy(&example_string_buffer).to_string();

    let html = Html(template.render()?);
    Ok((StatusCode::OK, html))
}
