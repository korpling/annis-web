use std::sync::Arc;

use crate::{
    client::search::FindQuery,
    converter::CSVExporter,
    errors::AppError,
    state::{ExportJob, GlobalAppState, SessionState},
    Result,
};
use axum::{
    body::StreamBody,
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse},
    Form,
};
use axum_sessions::extractors::ReadableSession;
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use minijinja::context;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;
use tokio_util::io::ReaderStream;

const DEFAULT_EXAMPLE: &str = r#"match number,1 node name,1 tiger::lemma,1 tiger::morph,1 tiger::pos
1,pcc2/11299#tok_1,Feigenblatt,Nom.Sg.Neut,NN
2,pcc2/11299#tok_2,der,Nom.Pl.*,ART
3,pcc2/11299#tok_3,jugendliche,Nom.Pl.*,NN"#;

#[derive(Deserialize, Debug)]
pub struct FormParams {
    query: Option<String>,
}

pub async fn show_page(
    session: ReadableSession,
    Query(params): Query<FormParams>,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    let example = if let Some(query) = params.query {
        create_example_output(query, &state, &session_state).await
    } else {
        Ok(DEFAULT_EXAMPLE.to_string())
    };
    let result = state
        .templates
        .get_template("export.html")?
        .render(context! {
            example,
            session => session_state,
            job => current_job(&session, &state),
        })?;

    Ok(Html(result))
}
pub async fn create_job(
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(params): Form<FormParams>,
) -> Result<impl IntoResponse> {
    let session_state: SessionState = session.get("state").unwrap_or_default();

    // Only allow one background job per session
    let session_id = session.id();
    app_state
        .background_jobs
        .entry(session_id.to_string())
        .or_insert_with(|| {
            // Create a background job that performs the export
            let find_query = FindQuery {
                query: params.query.unwrap_or_default(),
                corpora: session_state.selected_corpora.iter().cloned().collect(),
                query_language: QueryLanguage::AQL,
                limit: None,
                order: ResultOrder::Normal,
            };
            let app_state_copy = app_state.clone();
            let (sender, receiver) = channel(1);
            let handle: JoinHandle<Result<NamedTempFile>> = tokio::spawn(async move {
                let mut exporter = CSVExporter::new(find_query, Some(sender));
                let mut result_file = tempfile::NamedTempFile::new()?;

                exporter
                    .convert_text(&app_state_copy, None, &mut result_file)
                    .await?;
                Ok(result_file)
            });
            ExportJob::new(handle, receiver)
        });

    // Only render the export job status template
    let result = app_state
        .templates
        .get_template("export-job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

pub async fn job_status(
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let result = app_state
        .templates
        .get_template("export-job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

pub async fn cancel_job(
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_id = session.id();
    if let Some((_, job)) = app_state.background_jobs.remove(session_id) {
        job.handle.abort();
    }
    let result = app_state
        .templates
        .get_template("export-job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

pub async fn download_file(
    session: ReadableSession,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_id = session.id();

    if let Some((_, job)) = app_state.background_jobs.remove(session_id) {
        let file = job.handle.await??;
        let tokio_file = tokio::fs::File::open(file.path()).await?;
        let stream = ReaderStream::new(tokio_file);
        let body = StreamBody::new(stream);

        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/plain; charset=utf-8".parse()?);
        headers.insert(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"annis-export.csv\"".parse()?,
        );

        Ok((headers, body))
    } else {
        Err(AppError::DownloadFileNotFound)
    }
}

#[derive(Clone, Debug, Serialize)]
enum JobState {
    Idle,
    Running(f32),
    Finished,
}

fn current_job(session: &ReadableSession, app_state: &GlobalAppState) -> JobState {
    let session_id = session.id();
    if let Some(mut job) = app_state.background_jobs.get_mut(session_id) {
        if job.handle.is_finished() {
            JobState::Finished
        } else {
            JobState::Running(job.get_progress())
        }
    } else {
        JobState::Idle
    }
}

async fn create_example_output(
    query: String,
    state: &GlobalAppState,
    session_state: &SessionState,
) -> std::result::Result<String, String> {
    let example_query = FindQuery {
        query,
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };

    if !example_query.corpora.is_empty() && !example_query.query.is_empty() {
        let mut exporter = CSVExporter::new(example_query, None);
        let mut example_string_buffer = Vec::new();

        exporter
            .convert_text(state, Some(3), &mut example_string_buffer)
            .await
            .map_err(|e| format!("{}", e))?;
        let result = String::from_utf8_lossy(&example_string_buffer).to_string();
        Ok(result)
    } else {
        Ok(DEFAULT_EXAMPLE.to_string())
    }
}

#[cfg(test)]
mod tests;
