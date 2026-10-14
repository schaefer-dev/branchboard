//! A program running in a pseudo terminal: a Unix PTY, or `ConPTY` on Windows.
//! Output and the exit leave through [`PtySink`]; input, resizing and killing
//! go through [`Pty`]. Independent of Tauri.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Output arriving within this window is delivered as one chunk.
const COALESCE: Duration = Duration::from_millis(4);

/// Upper bound for one delivered chunk.
const MAX_CHUNK: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("The terminal could not be started.")]
    Spawn(String),

    #[error("The terminal has closed.")]
    Closed,

    #[error("Writing to the terminal failed.")]
    Io(#[from] std::io::Error),
}

/// The terminal size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    fn pty(self) -> PtySize {
        PtySize {
            rows: self.rows.max(1),
            cols: self.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// What to run. The environment starts as this process's environment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PtyCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Variables to set.
    pub env: Vec<(OsString, OsString)>,
    /// Variables to remove from the inherited environment.
    pub env_remove: Vec<OsString>,
    pub cwd: Option<PathBuf>,
}

impl PtyCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    #[must_use]
    pub fn env_remove(mut self, key: impl Into<OsString>) -> Self {
        self.env_remove.push(key.into());
        self
    }

    /// The value `key` gets, if this command sets it.
    pub fn env_value(&self, key: &str) -> Option<&OsString> {
        self.env
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    fn builder(&self) -> CommandBuilder {
        let mut builder = CommandBuilder::new(&self.program);
        builder.args(&self.args);
        for key in &self.env_remove {
            builder.env_remove(key);
        }
        for (key, value) in &self.env {
            builder.env(key, value);
        }
        if let Some(cwd) = &self.cwd {
            builder.cwd(cwd);
        }
        builder
    }
}

/// Receives what the program writes, then its exit.
pub trait PtySink: Send + Sync + 'static {
    fn output(&self, bytes: Vec<u8>);
    /// The program ended; `None` if it was killed or its status is unknown.
    /// Called once, after the last output.
    fn exited(&self, code: Option<u32>);
}

/// A running program. Dropping it kills the program.
pub struct Pty {
    shared: Arc<Shared>,
}

struct Shared {
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl Pty {
    pub fn spawn(
        command: &PtyCommand,
        size: Size,
        sink: Arc<dyn PtySink>,
    ) -> Result<Self, PtyError> {
        let spawn_error = |error: &dyn std::fmt::Display| PtyError::Spawn(error.to_string());
        let pair = native_pty_system()
            .openpty(size.pty())
            .map_err(|error| spawn_error(&error))?;
        let mut child = pair
            .slave
            .spawn_command(command.builder())
            .map_err(|error| spawn_error(&error))?;
        // Only the child keeps the terminal's slave side open, so reading
        // ends when it exits (on Unix).
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| spawn_error(&error))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| spawn_error(&error))?;
        let shared = Arc::new(Shared {
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(Some(writer)),
            killer: Mutex::new(child.clone_killer()),
        });

        let (chunks, received) = mpsc::channel::<Vec<u8>>();
        let (exit_tx, exit_rx) = mpsc::channel::<Option<u32>>();
        std::thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || read(reader, &chunks))?;
        let waiter = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("pty-waiter".into())
            .spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code());
                // `ConPTY` only ends the output once the pseudo console is
                // closed; closing it on Unix is harmless.
                waiter.close();
                let _ = exit_tx.send(code);
            })?;
        std::thread::Builder::new()
            .name("pty-output".into())
            .spawn(move || {
                forward(&received, &*sink);
                sink.exited(exit_rx.recv().ok().flatten());
            })?;
        Ok(Self { shared })
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), PtyError> {
        let mut writer = lock(&self.shared.writer);
        let writer = writer.as_mut().ok_or(PtyError::Closed)?;
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, size: Size) -> Result<(), PtyError> {
        let master = lock(&self.shared.master);
        let master = master.as_ref().ok_or(PtyError::Closed)?;
        master
            .resize(size.pty())
            .map_err(|error| PtyError::Spawn(error.to_string()))
    }

    /// Ends the program; the sink still gets its exit.
    pub fn kill(&self) {
        if let Err(error) = lock(&self.shared.killer).kill() {
            tracing::debug!(%error, "killing the terminal program failed");
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.kill();
    }
}

impl Shared {
    fn close(&self) {
        lock(&self.writer).take();
        lock(&self.master).take();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read(mut reader: Box<dyn Read + Send>, chunks: &mpsc::Sender<Vec<u8>>) {
    let mut buffer = vec![0; MAX_CHUNK];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                if chunks.send(buffer[..read].to_vec()).is_err() {
                    return;
                }
            }
        }
    }
}

/// Delivers output, merging what arrives within [`COALESCE`] of the first
/// byte into one chunk of at most [`MAX_CHUNK`] bytes.
fn forward(received: &mpsc::Receiver<Vec<u8>>, sink: &dyn PtySink) {
    while let Ok(mut chunk) = received.recv() {
        let deadline = Instant::now() + COALESCE;
        let mut open = true;
        while chunk.len() < MAX_CHUNK {
            let left = deadline.saturating_duration_since(Instant::now());
            match received.recv_timeout(left) {
                Ok(more) => chunk.extend_from_slice(&more),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        sink.output(chunk);
        if !open {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
