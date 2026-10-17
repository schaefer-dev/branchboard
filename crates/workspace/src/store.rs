//! Persists the [`Registry`] as a versioned JSON file.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::registry::Registry;

/// Current schema version of the file. Bump it together with a migration in
/// [`migrate`] whenever the stored format changes incompatibly.
const SCHEMA_VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredFile {
    version: u64,
    #[serde(flatten)]
    registry: Registry,
}

#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Loads the registry. A missing file yields an empty registry. A file that
    /// cannot be read is moved aside (never silently overwritten) and an empty
    /// registry is returned.
    pub fn load(&self) -> Registry {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Registry::default();
            }
            Err(error) => {
                tracing::error!(%error, path = %self.path.display(), "cannot read workspace file");
                self.quarantine();
                return Registry::default();
            }
        };
        match parse(&bytes) {
            Ok(registry) => registry,
            Err(reason) => {
                tracing::error!(%reason, path = %self.path.display(), "invalid workspace file");
                self.quarantine();
                Registry::default()
            }
        }
    }

    /// Writes the registry atomically: a temporary file is written next to
    /// the target and then renamed over it.
    pub async fn save(&self, registry: &Registry) -> std::io::Result<()> {
        let file = StoredFile {
            version: SCHEMA_VERSION,
            registry: registry.clone(),
        };
        let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
        if let Some(dir) = self.path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let temp = self.path.with_extension("json.tmp");
        tokio::fs::write(&temp, json).await?;
        tokio::fs::rename(&temp, &self.path).await
    }

    fn quarantine(&self) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let backup = self.path.with_extension(format!("json.broken-{stamp}"));
        match std::fs::rename(&self.path, &backup) {
            Ok(()) => {
                tracing::warn!(backup = %backup.display(), "moved unreadable workspace file aside");
            }
            Err(error) => tracing::error!(%error, "could not move unreadable workspace file aside"),
        }
    }
}

fn parse(bytes: &[u8]) -> Result<Registry, String> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .ok_or("missing schema version")?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "written by a newer Branchboard (schema {version}, supported {SCHEMA_VERSION})"
        ));
    }
    migrate(&mut value, version);
    let file: StoredFile = serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok(file.registry)
}

/// Upgrades an older file in place, one version step at a time. Add a step
/// per schema bump, e.g. `if from < 2 { /* rewrite fields */ }`.
fn migrate(_value: &mut Value, from: u64) {
    debug_assert!(from <= SCHEMA_VERSION);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("nested").join("workspace.json"));
        (dir, store)
    }

    #[tokio::test]
    async fn round_trips_the_registry() {
        let (_dir, store) = store();
        assert_eq!(store.load(), Registry::default());

        let mut registry = Registry::default();
        let id = registry.add_repo(PathBuf::from("/code/api")).id().clone();
        registry.create_group("Work").unwrap();
        registry.set_active_repo(Some(&id)).unwrap();
        store.save(&registry).await.unwrap();

        assert_eq!(store.load(), registry);
    }

    #[test]
    fn moves_corrupt_files_aside() {
        let (dir, store) = store();
        std::fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        std::fs::write(&store.path, b"{ not json").unwrap();

        assert_eq!(store.load(), Registry::default());
        assert!(!store.path.exists());
        let backups: Vec<_> = std::fs::read_dir(dir.path().join("nested"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".broken-"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn keeps_files_from_before_newer_settings() {
        let registry =
            parse(br#"{ "version": 1, "repos": [], "settings": { "fetchIntervalMinutes": 5 } }"#)
                .unwrap();
        assert_eq!(registry.settings.fetch_interval_minutes, 5);
        assert_eq!(
            registry.settings.terminal_font_size,
            crate::registry::DEFAULT_TERMINAL_FONT_SIZE
        );
        assert!(!registry.settings.sound_muted);
        assert!(registry.settings.mute_when_watching);
    }

    #[test]
    fn refuses_files_from_a_newer_version() {
        let result = parse(br#"{ "version": 99, "repos": [] }"#);
        assert!(result.unwrap_err().contains("newer"));
    }
}
