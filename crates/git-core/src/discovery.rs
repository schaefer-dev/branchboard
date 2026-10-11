//! Locating the git executable and building the `PATH` that git runs with.
//!
//! GUI apps on macOS do not inherit the user's shell environment, so tools
//! installed via Homebrew (git itself, git-lfs, credential helpers) would be
//! invisible. We therefore merge the login shell's `PATH` with the process
//! `PATH` and a few well-known install locations. The resulting search path
//! is used both to find git and as `PATH` for every git invocation.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::GitError;

/// How to find the git executable.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryOptions {
    /// Explicit path configured by the user. Takes precedence over the search path.
    pub explicit_path: Option<PathBuf>,
}

#[cfg(windows)]
const GIT_EXE: &str = "git.exe";
#[cfg(not(windows))]
const GIT_EXE: &str = "git";

pub(crate) fn locate_git(
    options: &DiscoveryOptions,
    search_path: &OsString,
) -> Result<PathBuf, GitError> {
    if let Some(explicit) = &options.explicit_path {
        return explicit
            .is_file()
            .then(|| explicit.clone())
            .ok_or(GitError::NotFound);
    }
    std::env::split_paths(search_path)
        .map(|dir| dir.join(GIT_EXE))
        .chain(well_known_executables())
        .find(|candidate| candidate.is_file())
        .ok_or(GitError::NotFound)
}

/// The `PATH` git is searched in and spawned with.
pub(crate) async fn effective_search_path() -> OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(shell_path) = login_shell_path().await {
        dirs.extend(std::env::split_paths(&shell_path));
    }
    if let Some(process_path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&process_path));
    }
    dirs.extend(well_known_dirs());

    let mut unique: Vec<PathBuf> = Vec::with_capacity(dirs.len());
    for dir in dirs {
        if !dir.as_os_str().is_empty() && !unique.contains(&dir) {
            unique.push(dir);
        }
    }
    // Only fails if an entry contains the platform's path separator, which
    // cannot happen for entries we just split on it.
    std::env::join_paths(unique).unwrap_or_default()
}

fn well_known_executables() -> impl Iterator<Item = PathBuf> {
    well_known_dirs().into_iter().map(|dir| dir.join(GIT_EXE))
}

#[cfg(target_os = "macos")]
fn well_known_dirs() -> Vec<PathBuf> {
    ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
fn well_known_dirs() -> Vec<PathBuf> {
    let program_files = ["ProgramW6432", "ProgramFiles"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| Path::new(&base).join("Git").join("cmd"));
    let per_user = std::env::var_os("LOCALAPPDATA")
        .map(|base| Path::new(&base).join("Programs").join("Git").join("cmd"));
    program_files.chain(per_user).collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn well_known_dirs() -> Vec<PathBuf> {
    vec![PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")]
}

/// Reads `PATH` from the user's login shell (macOS only).
#[cfg(target_os = "macos")]
async fn login_shell_path() -> Option<OsString> {
    use std::process::Stdio;
    use std::time::Duration;

    // Shell profiles may print arbitrary output, so the value is tagged with
    // a marker and taken from after its last occurrence.
    const MARKER: &str = "__BRANCHBOARD_PATH__";
    const TIMEOUT: Duration = Duration::from_secs(3);

    let shell = std::env::var_os("SHELL")
        .filter(|shell| Path::new(shell).is_absolute())
        .unwrap_or_else(|| OsString::from("/bin/zsh"));
    let output = tokio::process::Command::new(&shell)
        .args(["-l", "-c", &format!("printf '{MARKER}%s' \"$PATH\"")])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();

    let output = match tokio::time::timeout(TIMEOUT, output).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Ok(output)) => {
            tracing::warn!(status = %output.status, "login shell exited unsuccessfully");
            return None;
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to start login shell");
            return None;
        }
        Err(_) => {
            tracing::warn!(?TIMEOUT, "login shell timed out");
            return None;
        }
    };
    let stdout = String::from_utf8(output.stdout).ok()?;
    let path = stdout.rsplit_once(MARKER)?.1.trim();
    (!path.is_empty()).then(|| OsString::from(path))
}

#[cfg(not(target_os = "macos"))]
#[allow(clippy::unused_async)]
async fn login_shell_path() -> Option<OsString> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_path_must_exist() {
        let options = DiscoveryOptions {
            explicit_path: Some(PathBuf::from("/definitely/not/here/git")),
        };
        assert!(matches!(
            locate_git(&options, &OsString::new()),
            Err(GitError::NotFound)
        ));
    }

    #[test]
    fn finds_git_in_search_path() {
        let dir = tempfile::tempdir().unwrap();
        let fake_git = dir.path().join(GIT_EXE);
        std::fs::write(&fake_git, b"").unwrap();

        let search_path = std::env::join_paths([dir.path()]).unwrap();
        let found = locate_git(&DiscoveryOptions::default(), &search_path).unwrap();
        assert_eq!(found, fake_git);
    }
}
