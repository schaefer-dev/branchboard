//! Spawning git processes.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::GitError;

/// Options passed to every git invocation.
///
/// - `--no-optional-locks`: never take optional locks (e.g. the index refresh
///   in `git status`), so we do not race with editors like VS Code that poll
///   git in the background.
/// - `--literal-pathspecs`: paths are always file names, never glob patterns
///   (a file called `*.rs` must not match every Rust file).
/// - `core.quotepath=false`: report non-ASCII paths verbatim.
/// - `color.ui=never`: machine-readable output only.
const GLOBAL_ARGS: &[&str] = &[
    "--no-optional-locks",
    "--literal-pathspecs",
    "-c",
    "core.quotepath=false",
    "-c",
    "color.ui=never",
];

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Prevents a console window from flashing up for every git call on Windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Spawns git with a consistent environment. Cheap to clone.
#[derive(Debug, Clone)]
pub struct GitRunner {
    program: Arc<PathBuf>,
    search_path: Arc<OsString>,
}

impl GitRunner {
    pub fn new(program: PathBuf, search_path: OsString) -> Self {
        Self {
            program: Arc::new(program),
            search_path: Arc::new(search_path),
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The `PATH` git runs with (the login shell's on macOS), also useful to
    /// find other tools the user installed.
    pub fn search_path(&self) -> &OsStr {
        &self.search_path
    }

    /// Starts building a git invocation with the given arguments.
    pub fn command<I, S>(&self, args: I) -> GitCommand<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        GitCommand {
            runner: self,
            args: args
                .into_iter()
                .map(|arg| arg.as_ref().to_owned())
                .collect(),
            cwd: None,
            stdin: None,
            untranslated: false,
            timeout: DEFAULT_TIMEOUT,
            success_codes: &[0],
        }
    }
}

/// A single git invocation, created by [`GitRunner::command`].
#[derive(Debug)]
#[must_use = "a git command does nothing until `output` is awaited"]
pub struct GitCommand<'a> {
    runner: &'a GitRunner,
    args: Vec<OsString>,
    cwd: Option<PathBuf>,
    stdin: Option<Vec<u8>>,
    untranslated: bool,
    timeout: Duration,
    success_codes: &'static [i32],
}

/// Output of a git invocation that exited with an accepted code.
#[derive(Debug)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit_code: i32,
}

impl GitCommand<'_> {
    /// Streams NUL-delimited fields. Returning false stops and reaps git;
    /// dropping this future also kills it. Memory is bounded independently of
    /// history length (individual fields over 8 MiB are rejected).
    pub async fn visit_nul(
        self,
        mut visit: impl FnMut(&[u8]) -> Result<bool, GitError>,
    ) -> Result<(), GitError> {
        let mut command = tokio::process::Command::new(self.runner.program.as_path());
        command
            .args(GLOBAL_ARGS)
            .args(&self.args)
            .env("PATH", self.runner.search_path.as_os_str())
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        if self.untranslated {
            command.env("LC_ALL", "C");
        }
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(GitError::Spawn)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GitError::Parse("missing stdout pipe".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| GitError::Parse("missing stderr pipe".into()))?;
        let read = async {
            let mut reader = BufReader::new(stdout);
            let result = async {
                loop {
                    let mut field = Vec::new();
                    let count = (&mut reader)
                        .take(8 * 1024 * 1024 + 1)
                        .read_until(0, &mut field)
                        .await
                        .map_err(GitError::Spawn)?;
                    if count == 0 {
                        break;
                    }
                    if field.len() > 8 * 1024 * 1024 || field.pop() != Some(0) {
                        return Err(GitError::Parse("invalid or oversized history field".into()));
                    }
                    if !visit(&field)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            .await;
            // A parser error must also kill git before waiting for stderr:
            // otherwise a full stdout pipe can stall the drain until timeout.
            if !matches!(result, Ok(true)) {
                child.kill().await.map_err(GitError::Spawn)?;
                return result.map(|_| ());
            }
            let status = child.wait().await.map_err(GitError::Spawn)?;
            if !status.success() {
                return Err(GitError::Failed {
                    command: self.display_args(),
                    code: status.code(),
                    stderr: "History is unavailable; refresh to resolve the ref again.".into(),
                });
            }
            Ok(())
        };
        let drain = async {
            let mut reader = stderr;
            let mut buffer = [0; 4096];
            while reader.read(&mut buffer).await.unwrap_or(0) != 0 {}
        };
        tokio::time::timeout(self.timeout, async { tokio::join!(read, drain).0 })
            .await
            .map_err(|_| GitError::Timeout {
                command: self.display_args(),
                timeout: self.timeout,
            })?
    }

    /// Appends further arguments, e.g. paths after a `--` separator.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Feeds `input` to git's standard input.
    pub fn stdin(mut self, input: Vec<u8>) -> Self {
        self.stdin = Some(input);
        self
    }

    /// Passes `paths` via standard input (`--pathspec-from-file=-`), which
    /// avoids command line length limits for long path lists.
    pub fn pathspecs_from_stdin<I, S>(self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(path.as_ref().as_bytes());
            input.push(0);
        }
        self.args(["--pathspec-from-file=-", "--pathspec-file-nul"])
            .stdin(input)
    }

    /// Runs git with the C locale, for output that git would otherwise
    /// translate (e.g. `%(upstream:track)`).
    pub fn untranslated(mut self) -> Self {
        self.untranslated = true;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Exit codes treated as success. Defaults to `[0]`; some commands such as
    /// `git diff --no-index` use `1` to signal "differences found".
    pub fn success_codes(mut self, codes: &'static [i32]) -> Self {
        self.success_codes = codes;
        self
    }

    /// Runs git to completion. The process is killed if the returned future
    /// is dropped or the timeout elapses.
    pub async fn output(self) -> Result<GitOutput, GitError> {
        let mut command = tokio::process::Command::new(self.runner.program.as_path());
        command
            .args(GLOBAL_ARGS)
            .args(&self.args)
            .env("PATH", self.runner.search_path.as_os_str())
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        if self.untranslated {
            command.env("LC_ALL", "C");
        }
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let started = Instant::now();
        let mut child = command.spawn().map_err(GitError::Spawn)?;
        if let (Some(input), Some(mut pipe)) = (self.stdin.clone(), child.stdin.take()) {
            // Written concurrently so a large input cannot deadlock against
            // git filling its output pipes.
            tokio::spawn(async move {
                if let Err(error) = pipe.write_all(&input).await {
                    tracing::warn!(%error, "failed to write git stdin");
                }
            });
        }
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| GitError::Timeout {
                command: self.display_args(),
                timeout: self.timeout,
            })?
            .map_err(GitError::Spawn)?;

        let code = output.status.code();
        tracing::debug!(
            command = %self.display_args(),
            ?code,
            elapsed_ms = started.elapsed().as_millis(),
            "git"
        );

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        match code {
            Some(exit_code) if self.success_codes.contains(&exit_code) => Ok(GitOutput {
                stdout: output.stdout,
                stderr,
                exit_code,
            }),
            _ => Err(GitError::Failed {
                command: self.display_args(),
                code,
                stderr: stderr.trim().to_owned(),
            }),
        }
    }

    fn display_args(&self) -> String {
        let args: Vec<_> = self.args.iter().map(|arg| arg.to_string_lossy()).collect();
        args.join(" ")
    }
}
