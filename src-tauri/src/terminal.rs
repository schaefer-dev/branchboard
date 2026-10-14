//! The embedded terminal running the herdr client. There is at most one; it
//! starts when the frontend first shows it, and ends with the app. herdr's
//! server, and every agent in it, keeps running.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pty::{Pty, PtyCommand, PtySink, Size};
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::dto::TerminalExit;
use crate::error::AppError;

#[derive(Default)]
pub struct Terminal {
    current: Mutex<Option<Pty>>,
}

/// Delivers output as raw bytes (an `ArrayBuffer` in JavaScript) and the
/// exit as one JSON message `{ "exit": code }` after the last output.
struct ChannelSink(Channel<InvokeResponseBody>);

impl PtySink for ChannelSink {
    fn output(&self, bytes: Vec<u8>) {
        if let Err(error) = self.0.send(InvokeResponseBody::Raw(bytes)) {
            tracing::debug!(%error, "terminal output not delivered");
        }
    }

    fn exited(&self, code: Option<u32>) {
        let message = serde_json::to_string(&TerminalExit { exit: code })
            .expect("a terminal exit serializes");
        if let Err(error) = self.0.send(InvokeResponseBody::Json(message)) {
            tracing::debug!(%error, "terminal exit not delivered");
        }
    }
}

impl Terminal {
    /// Starts `command`, replacing a terminal that is still running (e.g.
    /// after the frontend reloaded). The replaced one reports its exit on its
    /// own channel, which nobody listens to any more.
    pub fn open(
        &self,
        command: &PtyCommand,
        size: Size,
        output: Channel<InvokeResponseBody>,
    ) -> Result<(), AppError> {
        let mut current = lock(&self.current);
        current.take();
        *current = Some(Pty::spawn(command, size, Arc::new(ChannelSink(output)))?);
        Ok(())
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), AppError> {
        let current = lock(&self.current);
        let pty = current.as_ref().ok_or(pty::PtyError::Closed)?;
        Ok(pty.write(bytes)?)
    }

    pub fn resize(&self, size: Size) -> Result<(), AppError> {
        let current = lock(&self.current);
        let pty = current.as_ref().ok_or(pty::PtyError::Closed)?;
        Ok(pty.resize(size)?)
    }

    /// Ends the terminal; herdr's server keeps running.
    pub fn close(&self) {
        lock(&self.current).take();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where the embedded client's herdr config is written.
pub fn config_file(dir: &Path) -> PathBuf {
    dir.join("herdr-embedded.toml")
}
