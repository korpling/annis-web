use std::{sync::Arc, time::Duration};

use crate::{state::GlobalAppState, Result};
use async_sqlx_session::SqliteSessionStore;
use axum::async_trait;
use axum_sessions::{
    async_session::{Session, SessionStore},
    SameSite, SessionLayer,
};
use tempfile::NamedTempFile;

#[derive(Clone, Debug)]
pub enum AnnisSessionStore {
    Sqlite { store: SqliteSessionStore },
    Temporary { store: SqliteSessionStore },
}

impl AnnisSessionStore {
    pub async fn create_temporary(global_state: Arc<GlobalAppState>) -> Result<AnnisSessionStore> {
        let session_file = NamedTempFile::new()?;
        let store = SqliteSessionStore::new(&format!(
            "sqlite://{}",
            session_file.path().to_string_lossy()
        ))
        .await?;
        store.migrate().await?;
        store.spawn_cleanup_task(Duration::from_secs(60 * 60));

        let session_layer = SessionLayer::new(
            store.clone(),
            "ginoh3ya5eiLi1nohph0equ6KiwicooweeNgovoojeQuaejaixiequah6eenoo2k".as_bytes(),
        )
        .with_same_site_policy(SameSite::Lax);

        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                global_state.cleanup(&store).await;
            }
        });
        todo!()
    }

    pub async fn load_session_by_id(
        &self,
        session_id: &str,
    ) -> axum_sessions::async_session::Result<Option<Session>> {
        match self {
            AnnisSessionStore::Sqlite { store, .. }
            | AnnisSessionStore::Temporary { store, .. } => {
                store.load_session(session_id.to_string()).await
            }
        }
    }
}

#[async_trait]
impl SessionStore for AnnisSessionStore {
    async fn load_session(
        &self,
        cookie_value: String,
    ) -> axum_sessions::async_session::Result<Option<axum_sessions::async_session::Session>> {
        match self {
            AnnisSessionStore::Sqlite { store, .. }
            | AnnisSessionStore::Temporary { store, .. } => store.load_session(cookie_value).await,
        }
    }

    async fn store_session(
        &self,
        session: axum_sessions::async_session::Session,
    ) -> axum_sessions::async_session::Result<Option<String>> {
        match self {
            AnnisSessionStore::Sqlite { store, .. }
            | AnnisSessionStore::Temporary { store, .. } => store.store_session(session).await,
        }
    }

    async fn destroy_session(
        &self,
        session: axum_sessions::async_session::Session,
    ) -> axum_sessions::async_session::Result {
        match self {
            AnnisSessionStore::Sqlite { store, .. }
            | AnnisSessionStore::Temporary { store, .. } => store.destroy_session(session).await,
        }
    }

    async fn clear_store(&self) -> axum_sessions::async_session::Result {
        match self {
            AnnisSessionStore::Sqlite { store, .. }
            | AnnisSessionStore::Temporary { store, .. } => store.clear_store().await,
        }
    }
}
