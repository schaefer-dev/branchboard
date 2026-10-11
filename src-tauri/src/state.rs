use git_core::{DiscoveryOptions, Git};
use tokio::sync::OnceCell;

use crate::error::AppError;

/// Application-wide state managed by Tauri.
#[derive(Default)]
pub struct AppState {
    git: OnceCell<Result<Git, AppError>>,
}

impl AppState {
    /// The git installation, discovered once on first access.
    pub async fn git(&self) -> Result<&Git, AppError> {
        self.git
            .get_or_init(|| async {
                Git::discover(DiscoveryOptions::default())
                    .await
                    .map_err(AppError::from)
            })
            .await
            .as_ref()
            .map_err(Clone::clone)
    }
}
