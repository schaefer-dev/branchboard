//! Events pushed from the backend to the frontend. Names must match
//! `ui/src/lib/api/events.ts`.

use tauri::{AppHandle, Emitter};
use workspace::{SummarySink, SummaryUpdate};

pub const REPO_SUMMARY: &str = "repo-summary";

pub struct TauriSummarySink(pub AppHandle);

impl SummarySink for TauriSummarySink {
    fn publish(&self, update: SummaryUpdate) {
        if let Err(error) = self.0.emit(REPO_SUMMARY, update) {
            tracing::warn!(%error, "failed to emit repo summary");
        }
    }
}
