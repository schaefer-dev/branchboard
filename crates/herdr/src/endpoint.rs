//! Where the herdr server listens.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::tokio::prelude::*;

/// The socket path of a herdr session. On Windows herdr serves a named pipe
/// whose name is this path; on other systems it is a Unix socket file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    path: PathBuf,
}

impl Endpoint {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Resolves the session the herdr CLI would talk to: `HERDR_SOCKET_PATH`,
    /// else the session named by `HERDR_SESSION`, else the default session in
    /// herdr's config directory. `None` without any home directory.
    pub fn from_env() -> Option<Self> {
        Self::resolve(|name| std::env::var_os(name))
    }

    fn resolve(var: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        if let Some(path) = var("HERDR_SOCKET_PATH") {
            return Some(Self::new(path));
        }
        let config = config_dir(&var)?;
        let dir = match var("HERDR_SESSION") {
            Some(name) => config.join("sessions").join(name),
            None => config,
        };
        Some(Self::new(dir.join("herdr.sock")))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) async fn connect(&self) -> io::Result<Stream> {
        // Named the same way herdr names its listener (both use interprocess).
        #[cfg(unix)]
        let name = {
            use interprocess::local_socket::{GenericFilePath, ToFsName as _};
            self.path.as_path().to_fs_name::<GenericFilePath>()?
        };
        #[cfg(windows)]
        let pipe = self.path.to_string_lossy().into_owned();
        #[cfg(windows)]
        let name = {
            use interprocess::local_socket::{GenericNamespaced, ToNsName as _};
            pipe.as_str().to_ns_name::<GenericNamespaced>()?
        };
        Stream::connect(name).await
    }
}

/// herdr's config directory, following herdr's own rules.
fn config_dir(var: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    const APP: &str = "herdr";
    if let Some(dir) = var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join(APP));
    }
    if cfg!(windows) {
        if let Some(dir) = var("APPDATA") {
            return Some(PathBuf::from(dir).join(APP));
        }
        if let Some(profile) = var("USERPROFILE") {
            return Some(
                PathBuf::from(profile)
                    .join("AppData")
                    .join("Roaming")
                    .join(APP),
            );
        }
    }
    var("HOME").map(|home| PathBuf::from(home).join(".config").join(APP))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(vars: &[(&str, &str)]) -> Option<PathBuf> {
        Endpoint::resolve(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
        .map(|endpoint| endpoint.path)
    }

    #[test]
    fn explicit_socket_path_wins() {
        let path = resolve(&[
            ("HERDR_SOCKET_PATH", "/run/custom.sock"),
            ("HERDR_SESSION", "work"),
            ("HOME", "/home/me"),
        ]);
        assert_eq!(path, Some(PathBuf::from("/run/custom.sock")));
    }

    #[test]
    fn named_session_lives_below_sessions() {
        let path = resolve(&[("HERDR_SESSION", "work"), ("XDG_CONFIG_HOME", "/cfg")]);
        let expected = Path::new("/cfg/herdr")
            .join("sessions")
            .join("work")
            .join("herdr.sock");
        assert_eq!(path, Some(expected));
    }

    #[test]
    fn default_session_in_config_dir() {
        let path = resolve(&[("HOME", "/home/me"), ("APPDATA", "/appdata")]);
        let expected = if cfg!(windows) {
            Path::new("/appdata").join("herdr").join("herdr.sock")
        } else {
            Path::new("/home/me/.config/herdr").join("herdr.sock")
        };
        assert_eq!(path, Some(expected));
    }

    #[test]
    fn nothing_without_a_home() {
        assert_eq!(resolve(&[]), None);
    }
}
