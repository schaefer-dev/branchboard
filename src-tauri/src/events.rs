//! Events pushed from the backend to the frontend. Names must match
//! `ui/src/lib/api/events.ts`.

use std::sync::Arc;

use herdr::{HerdrSink, Session};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use workspace::{EventSink, FetchUpdate, StatusUpdate, SummaryUpdate};

use crate::state::AppState;

pub const REPO_SUMMARY: &str = "repo-summary";
pub const REPO_STATUS: &str = "repo-status";
pub const REPO_FETCH: &str = "repo-fetch";
pub const REPO_AGENTS: &str = "repo-agents";

pub struct TauriEvents<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> TauriEvents<R> {
    fn emit(&self, event: &str, payload: impl Serialize + Clone) {
        emit(&self.0, event, payload);
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, event: &str, payload: impl Serialize + Clone) {
    if let Err(error) = app.emit(event, payload) {
        tracing::warn!(%error, event, "failed to emit event");
    }
}

impl<R: Runtime> EventSink for TauriEvents<R> {
    fn summary(&self, update: SummaryUpdate) {
        self.emit(REPO_SUMMARY, update);
    }

    fn status(&self, update: StatusUpdate) {
        self.emit(REPO_STATUS, update);
    }

    fn fetch(&self, update: FetchUpdate) {
        self.emit(REPO_FETCH, update);
    }
}

impl<R: Runtime> HerdrSink for TauriEvents<R> {
    fn session_changed(&self, _session: Option<Arc<Session>>) {
        // Agents are reported per repository, which needs the repository
        // list; work that out off the caller's task.
        let app = self.0.clone();
        tauri::async_runtime::spawn(async move {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            state
                .publish_agents(|agents| emit(&app, REPO_AGENTS, agents))
                .await;
        });
    }
}
