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
    response::IntoResponse,
    Form,
};
use axum_sessions::extractors::WritableSession;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use serde::Deserialize;
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;

const DEFAULT_EXAMPLE: &str = r#"match number,1 node name,1 tiger::lemma,1 tiger::morph,1 tiger::pos
1,pcc2/11299#tok_1,Feigenblatt,Nom.Sg.Neut,NN
2,pcc2/11299#tok_2,der,Nom.Pl.*,ART
3,pcc2/11299#tok_3,jugendliche,Nom.Pl.*,NN"#;

#[derive(Template, Debug)]
#[template(path = "export-example-output.html")]
struct ExampleOutputTemplate {
    example: String,
    error: Option<String>,
}

impl Default for ExampleOutputTemplate {
    fn default() -> Self {
        Self {
            example: DEFAULT_EXAMPLE.to_string(),
            error: None,
        }
    }
}

#[derive(Template, Debug)]
#[template(path = "export-job.html")]
struct ExportJobTemplate {
    url_prefix: String,
    job: Option<uuid::Uuid>,
}

#[derive(Template, Debug)]
#[template(path = "export.html")]
struct Export {
    url_prefix: String,
    state: SessionState,
    example_output: ExampleOutputTemplate,
    export_job: ExportJobTemplate,
}

#[derive(Deserialize, Debug)]
pub struct FormParams {
    query: Option<String>,
}

pub async fn get(
    session: WritableSession,
    Query(params): Query<FormParams>,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let example_output = if let Some(query) = params.query {
        create_example_output_template(query, &state, &session_state).await?
    } else {
        ExampleOutputTemplate::default()
    };
    let template = Export {
        url_prefix: state.frontend_prefix.to_string(),
        example_output,
        state: session_state,
        export_job: ExportJobTemplate {
            url_prefix: state.frontend_prefix.to_string(),
            job: None,
        },
    };

    Ok(template)
}
pub async fn run(
    mut session: WritableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(params): Form<FormParams>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    // Create a background job that performs the export
    let find_query = FindQuery {
        query: params.query.unwrap_or_default().clone(),
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::Normal,
    };
    let app_state_copy = app_state.clone();
    let (sender, receiver) = channel(1);
    let handle: JoinHandle<Result<String>> = tokio::spawn(async move {
        let mut exporter = CSVExporter::new(find_query, sender);
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

    let template = ExportJobTemplate {
        url_prefix: app_state.frontend_prefix.to_string(),
        job: Some(handle_id),
    };

    Ok(template)
}

async fn create_example_output_template(
    query: String,
    state: &GlobalAppState,
    session_state: &SessionState,
) -> Result<ExampleOutputTemplate> {
    let example_query = FindQuery {
        query,
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };

    let mut template = ExampleOutputTemplate::default();

    if !example_query.corpora.is_empty() && !example_query.query.is_empty() {
        let (sender, _receiver) = channel(1);
        let mut exporter = CSVExporter::new(example_query, sender);
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
