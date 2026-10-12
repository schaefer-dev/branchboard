//! Events pushed from the backend to the frontend. Names must match
//! `ui/src/lib/api/events.ts`.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use workspace::{EventSink, StatusUpdate, SummaryUpdate};

pub const REPO_SUMMARY: &str = "repo-summary";
pub const REPO_STATUS: &str = "repo-status";

pub struct TauriEvents<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> TauriEvents<R> {
    fn emit(&self, event: &str, payload: impl Serialize + Clone) {
        if let Err(error) = self.0.emit(event, payload) {
            tracing::warn!(%error, event, "failed to emit event");
        }
    }
}

impl<R: Runtime> EventSink for TauriEvents<R> {
    fn summary(&self, update: SummaryUpdate) {
        self.emit(REPO_SUMMARY, update);
    }

    fn status(&self, update: StatusUpdate) {
        self.emit(REPO_STATUS, update);
    }
}
