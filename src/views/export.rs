use std::{collections::HashMap, sync::Arc};

use crate::{
    client::{self, search::FindQuery},
    converter::{CSVConfig, CSVExporter},
    errors::AppError,
    state::{ExportJob, GlobalAppState, SessionArg, SessionState},
    Result,
};
use axum::{
    body::StreamBody,
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
    Form, Router,
};
use graphannis::corpusstorage::{QueryLanguage, ResultOrder};
use minijinja::context;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;
use tokio_util::io::ReaderStream;
use tower_sessions::Session;

const DEFAULT_EXAMPLE: &str = r#"text,tiger::lemma (1),tiger::morph (1),tiger::pos (1)
Feigenblatt,Feigenblatt,Nom.Sg.Neut,NN
die,der,Nom.Pl.*,ART
Jugendlichen,jugendliche,Nom.Pl.*,NN"#;

pub fn create_routes() -> Result<Router<Arc<GlobalAppState>>> {
    let result = Router::new()
        .route("/", get(show_page))
        .route("/job", post(create_job))
        .route("/job", get(job_status))
        .route("/job", delete(cancel_job))
        .route("/file", get(download_file));
    Ok(result)
}

#[derive(Deserialize, Debug)]
struct FormParams {
    query: Option<String>,
    #[serde(flatten)]
    config: CSVConfig,
}

async fn show_page(
    session: Session,
    session_state: SessionState,
    Query(params): Query<FormParams>,
    State(state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let example = if let Some(query) = &params.query {
        create_example_output(query, &params, &state, &session, &session_state).await
    } else {
        Ok(DEFAULT_EXAMPLE.to_string())
    };

    let default_context_sizes = vec![0, 1, 5, 10, 20];

    // Find all segmentations that exist in all of the corpora
    let number_collected_corpora = session_state.selected_corpora.len();
    let mut all_segmentations: HashMap<String, usize> = HashMap::new();

    for corpus in session_state.selected_corpora.iter() {
        if let Ok(corpus_segmentations) =
            client::corpora::segmentations(&SessionArg::Session(session.clone()), corpus, &state)
                .await
        {
            for seg in corpus_segmentations {
                let entry = all_segmentations.entry(seg).or_insert(0);
                *entry += 1;
            }
        }
    }

    let segmentations: Vec<_> = all_segmentations
        .into_iter()
        .filter_map(|(k, v)| {
            if v == number_collected_corpora {
                Some(k)
            } else {
                None
            }
        })
        .collect();

    let result = state
        .templates
        .get_template("export.html")?
        .render(context! {
            example,
            session => session_state,
            job => current_job(&session, &state),
            config => params.config,
            default_context_sizes,
            segmentations,
        })?;

    Ok(Html(result))
}

async fn create_job(
    session: Session,
    session_state: SessionState,
    State(app_state): State<Arc<GlobalAppState>>,
    Form(params): Form<FormParams>,
) -> Result<impl IntoResponse> {
    // Only allow one background job per session
    let session_arg = SessionArg::Id(session.id().to_string());
    app_state
        .background_jobs
        .entry(session_arg.id().to_string())
        .or_insert_with(|| {
            // Create a background job that performs the export
            let find_query = FindQuery {
                query: params.query.unwrap_or_default(),
                corpora: session_state.selected_corpora.iter().cloned().collect(),
                query_language: QueryLanguage::AQL,
                limit: None,
                order: ResultOrder::Normal,
            };
            let config = params.config;
            let app_state_copy = app_state.clone();
            let (sender, receiver) = channel(1);
            let handle: JoinHandle<Result<NamedTempFile>> = tokio::spawn(async move {
                let mut exporter = CSVExporter::new(find_query, config, Some(sender));
                let mut result_file = tempfile::NamedTempFile::new()?;

                exporter
                    .convert_text(session_arg, &app_state_copy, None, &mut result_file)
                    .await?;
                Ok(result_file)
            });
            ExportJob::new(handle, receiver)
        });

    // Only render the export job status template
    let result = app_state
        .templates
        .get_template("export/job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

async fn job_status(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let result = app_state
        .templates
        .get_template("export/job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

async fn cancel_job(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_id = session.id().to_string();
    if let Some((_, job)) = app_state.background_jobs.remove(&session_id) {
        job.handle.abort();
    }
    let result = app_state
        .templates
        .get_template("export/job.html")?
        .render(context! {
            job => current_job(&session, &app_state),
        })?;

    Ok(Html(result))
}

async fn download_file(
    session: Session,
    State(app_state): State<Arc<GlobalAppState>>,
) -> Result<impl IntoResponse> {
    let session_id = session.id().to_string();

    if let Some((_, job)) = app_state.background_jobs.remove(&session_id) {
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

fn current_job(session: &Session, app_state: &GlobalAppState) -> JobState {
    let session_id = session.id().to_string();
    if let Some(mut job) = app_state.background_jobs.get_mut(&session_id) {
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
    query: &str,
    params: &FormParams,
    state: &GlobalAppState,
    session: &Session,
    session_state: &SessionState,
) -> std::result::Result<String, String> {
    let example_query = FindQuery {
        query: query.to_string(),
        corpora: session_state.selected_corpora.iter().cloned().collect(),
        query_language: QueryLanguage::AQL,
        limit: None,
        order: ResultOrder::NotSorted,
    };
    let config = params.config.clone();

    if !example_query.corpora.is_empty() && !example_query.query.is_empty() {
        let mut exporter = CSVExporter::new(example_query, config, None);
        let mut example_string_buffer = Vec::new();

        exporter
            .convert_text(
                SessionArg::Session(session.to_owned()),
                state,
                Some(3),
                &mut example_string_buffer,
            )
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
