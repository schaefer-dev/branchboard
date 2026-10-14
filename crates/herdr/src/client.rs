//! Running the herdr client (its terminal UI) inside Branchboard.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use pty::PtyCommand;
use toml_edit::{DocumentMut, Item, value};

use crate::Endpoint;

#[cfg(windows)]
const BINARY: &str = "herdr.exe";
#[cfg(not(windows))]
const BINARY: &str = "herdr";

/// Variables a herdr pane sets for programs inside it. Branchboard may be
/// started from such a pane; the embedded client must not inherit them.
const PANE_VARIABLES: &[&str] = &[
    "HERDR_PANE_ID",
    "HERDR_TAB_ID",
    "HERDR_WORKSPACE_ID",
    "HERDR_ENV",
    "HERDR_BIN_PATH",
    "HERDR_SESSION",
    "HERDR_CLIENT_SOCKET_PATH",
];

/// Finds the herdr binary in `search_path` or where its installers put it.
pub fn find_binary(search_path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .map(|dir| dir.join(BINARY))
        .chain(install_locations())
        .find(|candidate| candidate.is_file())
}

fn install_locations() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut locations: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            locations.push(Path::new(&local).join("Programs").join("Herdr").join("bin"));
        }
    } else {
        // install.sh, Homebrew (Apple silicon and Intel), Nix, Cargo.
        if let Some(home) = &home {
            locations.push(home.join(".local").join("bin"));
        }
        locations.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
        if let Some(home) = &home {
            locations.push(home.join(".nix-profile").join("bin"));
            locations.push(home.join(".cargo").join("bin"));
        }
    }
    locations.into_iter().map(|dir| dir.join(BINARY)).collect()
}

/// The config file herdr reads for the user: `HERDR_CONFIG_PATH`, else
/// `config.toml` in herdr's config directory.
pub fn user_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HERDR_CONFIG_PATH") {
        return Some(PathBuf::from(path));
    }
    crate::endpoint::config_dir(&|name: &str| std::env::var_os(name))
        .map(|dir| dir.join("config.toml"))
}

/// The user's herdr config adjusted for the embedded client: herdr's own
/// sidebar is hidden, since Branchboard shows the repositories and agents.
/// Everything else (keys, theme, …) stays the user's. An unreadable config
/// is replaced by one with only these settings, like herdr's defaults.
pub fn embedded_config(user: Option<&str>) -> String {
    let mut document = user
        .and_then(|text| {
            text.parse::<DocumentMut>()
                .inspect_err(|error| tracing::warn!(%error, "ignoring an invalid herdr config"))
                .ok()
        })
        .unwrap_or_default();
    if !document.contains_key("onboarding") {
        document["onboarding"] = value(false);
    }
    let ui = document
        .entry("ui")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut();
    if let Some(ui) = ui {
        ui.insert("sidebar_start_collapsed", Item::Value(true.into()));
        ui.insert("sidebar_collapsed_mode", Item::Value("hidden".into()));
    } else {
        // `ui` was not a table; herdr would reject that anyway.
        document["ui"] = toml_edit::table();
        document["ui"]["sidebar_start_collapsed"] = value(true);
        document["ui"]["sidebar_collapsed_mode"] = value("hidden");
    }
    document.to_string()
}

/// How to start the embedded herdr client.
#[derive(Debug, Clone)]
pub struct EmbeddedClient {
    pub binary: PathBuf,
    /// The session the client attaches to (started if needed).
    pub endpoint: Endpoint,
    /// The file written from [`embedded_config`].
    pub config: PathBuf,
}

impl EmbeddedClient {
    /// The command line and environment. `lang` is this process's `LANG`;
    /// apps started from the macOS Dock have none, which would make herdr
    /// assume a non-UTF-8 terminal.
    pub fn command(&self, lang: Option<&OsStr>) -> PtyCommand {
        let mut command = PtyCommand::new(&self.binary)
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .env("TERM_PROGRAM", "Branchboard")
            .env("HERDR_SOCKET_PATH", self.endpoint.path())
            .env("HERDR_CONFIG_PATH", &self.config);
        if lang.is_none_or(OsStr::is_empty) {
            command = command.env("LANG", "en_US.UTF-8");
        }
        for variable in PANE_VARIABLES {
            command = command.env_remove(OsString::from(variable));
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_binary_in_the_search_path() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join(BINARY);
        std::fs::write(&binary, b"").unwrap();
        let search_path = std::env::join_paths([Path::new("/nowhere"), dir.path()]).unwrap();
        assert_eq!(find_binary(&search_path), Some(binary));
    }

    #[test]
    fn hides_the_sidebar_and_keeps_the_rest() {
        let user = "\
# my herdr
onboarding = true

[theme]
name = \"one-dark\"

[ui]
sidebar_width = 30
sidebar_start_collapsed = false

[keys]
prefix = \"ctrl+a\"
";
        let config = embedded_config(Some(user));
        let parsed: DocumentMut = config.parse().unwrap();
        assert_eq!(
            parsed["ui"]["sidebar_start_collapsed"].as_bool(),
            Some(true)
        );
        assert_eq!(
            parsed["ui"]["sidebar_collapsed_mode"].as_str(),
            Some("hidden")
        );
        assert_eq!(parsed["ui"]["sidebar_width"].as_integer(), Some(30));
        assert_eq!(parsed["theme"]["name"].as_str(), Some("one-dark"));
        assert_eq!(parsed["keys"]["prefix"].as_str(), Some("ctrl+a"));
        assert_eq!(parsed["onboarding"].as_bool(), Some(true));
        assert!(config.starts_with("# my herdr"));
    }

    #[test]
    fn works_without_a_user_config() {
        for user in [None, Some("this is [not toml")] {
            let parsed: DocumentMut = embedded_config(user).parse().unwrap();
            assert_eq!(
                parsed["ui"]["sidebar_collapsed_mode"].as_str(),
                Some("hidden")
            );
            assert_eq!(parsed["onboarding"].as_bool(), Some(false));
        }
    }

    #[test]
    fn the_client_talks_to_the_linked_session() {
        let client = EmbeddedClient {
            binary: PathBuf::from("/bin/herdr"),
            endpoint: Endpoint::new("/run/herdr.sock"),
            config: PathBuf::from("/data/herdr-embedded.toml"),
        };
        let command = client.command(None);
        assert_eq!(command.program, Path::new("/bin/herdr"));
        assert_eq!(command.args, Vec::<OsString>::new());
        let get = |key: &str| command.env_value(key).and_then(|value| value.to_str());
        assert_eq!(get("HERDR_SOCKET_PATH"), Some("/run/herdr.sock"));
        assert_eq!(get("HERDR_CONFIG_PATH"), Some("/data/herdr-embedded.toml"));
        assert_eq!(get("TERM"), Some("xterm-256color"));
        assert_eq!(get("LANG"), Some("en_US.UTF-8"));
        assert!(command.env_remove.iter().any(|key| key == "HERDR_PANE_ID"));

        let with_lang = client.command(Some(OsStr::new("de_DE.UTF-8")));
        assert_eq!(with_lang.env_value("LANG"), None);
    }
}
