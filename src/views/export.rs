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
    Form,
};
use axum_sessions::extractors::WritableSession;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use serde::Deserialize;
use tokio::task::JoinHandle;
use tracing::info;

const DEFAULT_EXAMPLE: &str = r#"match number,1 node name,1 tiger::lemma,1 tiger::morph,1 tiger::pos
1,pcc2/11299#tok_1,Feigenblatt,Nom.Sg.Neut,NN
2,pcc2/11299#tok_2,der,Nom.Pl.*,ART
3,pcc2/11299#tok_3,jugendliche,Nom.Pl.*,NN"#;

#[derive(Template, Debug)]
#[template(path = "export-example-output.html")]
struct ExampleOutput {
    example: String,
    error: Option<String>,
}

impl Default for ExampleOutput {
    fn default() -> Self {
        Self {
            example: DEFAULT_EXAMPLE.to_string(),
            error: None,
        }
    }
}

#[derive(Debug)]
struct ExportStatus {
    progress: f32,
    messages: String,
}

#[derive(Template, Debug)]
#[template(path = "export.html")]
struct Export {
    url_prefix: String,
    state: SessionState,
    example_output: ExampleOutput,
    export_status: Option<ExportStatus>,
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
        export_status: None,
    };

    Ok(template)
}
#[derive(Deserialize, Debug)]
pub struct StartExportParams {
    query: String,
}

pub async fn start_export(
    mut session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(params): Form<StartExportParams>,
) -> Result<impl IntoResponse> {
    let mut export_status = None;
    let session_state: SessionState = session.get("state").unwrap_or_default();

    info!("Export requested");
    export_status = Some(ExportStatus {
        progress: 0.0,
        messages: "Export started".to_string(),
    });

    // Create a background job that performs the export
    let find_query = FindQuery {
        query: params.query.clone(),
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::Normal,
    };
    let app_state_copy = app_state.clone();
    let handle: JoinHandle<Result<String>> = tokio::spawn(async move {
        let mut exporter = CSVExporter::new(find_query);
        let mut result_string_buffer = Vec::new();

        exporter
            .convert_text(&app_state_copy, Some(3), &mut result_string_buffer)
            .await?;
        let result = String::from_utf8_lossy(&result_string_buffer).to_string();
        Ok(result)
    });
    // Store the join handle in the global application job pool and remember
    // its ID in the session state
    let handle_id = uuid::Uuid::new_v4();
    app_state.background_jobs.insert(handle_id, handle);
    session.insert("export-handle-id", handle_id.as_u128())?;

    let template = Export {
        url_prefix: app_state.frontend_prefix.to_string(),
        example_output: create_example_output_template(params.query, &app_state, &session_state)
            .await?,
        state: session_state.clone(),
        export_status,
    };
    Ok(template)
}

async fn create_example_output_template(
    query: String,
    state: &GlobalAppState,
    session_state: &SessionState,
) -> Result<ExampleOutput> {
    let example_query = FindQuery {
        query,
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };

    let mut template = ExampleOutput::default();

    if !example_query.corpora.is_empty() && !example_query.query.is_empty() {
        let mut exporter = CSVExporter::new(example_query);
        let mut example_string_buffer = Vec::new();
        match exporter
            .convert_text(state, Some(3), &mut example_string_buffer)
            .await
        {
            Ok(_) => template.example = String::from_utf8_lossy(&example_string_buffer).to_string(),
            Err(e) => template.error = Some(format!("{}", e)),
        }
    }
    Ok(template)
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
    let template = create_example_output_template(params.query, &state, &session_state).await?;
    Ok(template)
}
