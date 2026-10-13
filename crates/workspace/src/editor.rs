//! Opening repositories and files in the user's editor (VS Code).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::WorkspaceError;

/// How to launch the editor: a command line program such as VS Code's `code`.
#[derive(Debug, Clone)]
pub struct Editor {
    program: PathBuf,
}

#[cfg(windows)]
const CLI_NAMES: &[&str] = &["code.cmd", "code.exe"];
#[cfg(not(windows))]
const CLI_NAMES: &[&str] = &["code"];

impl Editor {
    /// Uses `program` as the editor command.
    pub fn custom(program: PathBuf) -> Self {
        Self { program }
    }

    /// Finds VS Code's command line launcher in `search_path` or in the
    /// standard install locations.
    pub fn detect(search_path: &OsStr) -> Option<Self> {
        let in_path = std::env::split_paths(search_path)
            .flat_map(|dir| CLI_NAMES.iter().map(move |name| dir.join(name)));
        in_path
            .chain(install_locations())
            .find(|candidate| candidate.is_file())
            .map(|program| Self { program })
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Opens `repo` in a window, and `file` (absolute) at `line` if given.
    /// Returns once the launcher started; it is not waited for.
    pub fn open(
        &self,
        repo: &Path,
        file: Option<(&Path, Option<u32>)>,
    ) -> Result<(), WorkspaceError> {
        let mut args: Vec<OsString> = vec![repo.as_os_str().to_owned()];
        if let Some((file, line)) = file {
            let mut target = file.as_os_str().to_owned();
            if let Some(line) = line {
                target.push(format!(":{line}"));
            }
            args.extend([OsString::from("--goto"), target]);
        }
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = command
            .spawn()
            .map_err(|error| WorkspaceError::Editor(error.to_string()))?;
        // Reap the launcher when it exits.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn install_locations() -> Vec<PathBuf> {
    let bundle = "Visual Studio Code.app/Contents/Resources/app/bin/code";
    let mut locations = vec![Path::new("/Applications").join(bundle)];
    if let Some(home) = std::env::var_os("HOME") {
        locations.push(Path::new(&home).join("Applications").join(bundle));
    }
    locations
}

#[cfg(windows)]
fn install_locations() -> Vec<PathBuf> {
    let user = std::env::var_os("LOCALAPPDATA")
        .map(|base| Path::new(&base).join("Programs").join("Microsoft VS Code"));
    let system = ["ProgramFiles", "ProgramW6432"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| Path::new(&base).join("Microsoft VS Code"));
    user.into_iter()
        .chain(system)
        .map(|dir| dir.join("bin").join("code.cmd"))
        .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn install_locations() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_launcher_in_the_search_path() {
        let dir = tempfile::tempdir().unwrap();
        let launcher = dir.path().join(CLI_NAMES[0]);
        std::fs::write(&launcher, b"").unwrap();
        let search_path = std::env::join_paths([dir.path()]).unwrap();
        let editor = Editor::detect(&search_path).unwrap();
        assert_eq!(editor.program(), launcher);
    }
}
