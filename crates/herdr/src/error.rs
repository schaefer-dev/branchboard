use std::io;

/// Errors talking to herdr. Messages are written for end users.
#[derive(Debug, thiserror::Error)]
pub enum HerdrError {
    #[error("herdr is not running. Start herdr and try again.")]
    NotRunning,

    #[error("herdr did not respond in time.")]
    Timeout,

    #[error("herdr did not respond as expected.")]
    Protocol(String),

    #[error("herdr reported an error.")]
    Api { code: String, message: String },

    #[error("The connection to herdr failed.")]
    Io(#[source] io::Error),
}

impl HerdrError {
    /// Classifies a failed connection attempt: no socket, or a stale one
    /// left behind by a server that is gone, means herdr is not running.
    pub(crate) fn connect(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => Self::NotRunning,
            _ => Self::Io(error),
        }
    }

    /// Technical details for logs and the error's detail line.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::NotRunning | Self::Timeout => None,
            Self::Protocol(detail) => Some(detail.clone()),
            Self::Api { code, message } => Some(format!("{code}: {message}")),
            Self::Io(error) => Some(error.to_string()),
        }
    }
}
