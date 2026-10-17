//! The list of repositories and groups the user works with.
//!
//! Pure data and rules, no I/O: every operation validates its input and
//! either applies completely or returns an error without changing anything.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::WorkspaceError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RepoId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GroupId(String);

impl RepoId {
    fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl GroupId {
    fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub id: RepoId,
    /// Root of the working tree.
    pub path: PathBuf,
    /// Display name chosen by the user; the folder name is used otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<GroupId>,
    #[serde(default)]
    pub favorite: bool,
    /// Ref the branches are compared with, if the user chose one; otherwise
    /// [`DEFAULT_MAINLINE`], [`FALLBACK_MAINLINE`] or [`FINAL_FALLBACK_MAINLINE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mainline: Option<String>,
}

/// Branches are compared with this ref unless a repository overrides it…
pub const DEFAULT_MAINLINE: &str = "origin/development";
/// …or with this one if the repository has no [`DEFAULT_MAINLINE`].
pub const FALLBACK_MAINLINE: &str = "origin/main";
/// …or with this one if neither of the above exists.
pub const FINAL_FALLBACK_MAINLINE: &str = "origin/master";

/// Background fetch interval unless the user changes it.
pub const DEFAULT_FETCH_INTERVAL_MINUTES: u32 = 10;

/// Font size of the embedded terminal in CSS pixels (= points on macOS).
pub const DEFAULT_TERMINAL_FONT_SIZE: u16 = 17;
/// The range the terminal font size is kept in.
pub const TERMINAL_FONT_SIZES: std::ops::RangeInclusive<u16> = 9..=32;

impl RepoEntry {
    pub fn display_name(&self) -> String {
        self.custom_name
            .clone()
            .unwrap_or_else(|| folder_name(&self.path))
    }

    /// The mainline the user chose for this repository, if any.
    pub fn mainline(&self) -> Option<&str> {
        self.mainline.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    /// Minutes between background fetches of the active repository; 0 turns
    /// background fetching off.
    pub fetch_interval_minutes: u32,
    /// Font size of the embedded terminal; files from before it existed get
    /// the default.
    #[serde(default = "default_terminal_font_size")]
    pub terminal_font_size: u16,
    /// No sounds when agents need the user; notifications still show.
    #[serde(default)]
    pub sound_muted: bool,
    /// No sound for an agent the user is watching in the terminal.
    #[serde(default = "default_mute_when_watching")]
    pub mute_when_watching: bool,
}

fn default_terminal_font_size() -> u16 {
    DEFAULT_TERMINAL_FONT_SIZE
}

fn default_mute_when_watching() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fetch_interval_minutes: DEFAULT_FETCH_INTERVAL_MINUTES,
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            sound_muted: false,
            mute_when_watching: default_mute_when_watching(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(default)]
    pub repos: Vec<RepoEntry>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_repo_id: Option<RepoId>,
    #[serde(default)]
    pub settings: Settings,
}

/// Result of [`Registry::add_repo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    New(RepoId),
    /// The path was already registered.
    Existing(RepoId),
}

impl Added {
    pub fn id(&self) -> &RepoId {
        match self {
            Self::New(id) | Self::Existing(id) => id,
        }
    }
}

impl Registry {
    pub fn repo(&self, id: &RepoId) -> Result<&RepoEntry, WorkspaceError> {
        self.repos
            .iter()
            .find(|repo| &repo.id == id)
            .ok_or(WorkspaceError::RepoNotFound)
    }

    fn repo_mut(&mut self, id: &RepoId) -> Result<&mut RepoEntry, WorkspaceError> {
        self.repos
            .iter_mut()
            .find(|repo| &repo.id == id)
            .ok_or(WorkspaceError::RepoNotFound)
    }

    fn group_mut(&mut self, id: &GroupId) -> Result<&mut Group, WorkspaceError> {
        self.groups
            .iter_mut()
            .find(|group| &group.id == id)
            .ok_or(WorkspaceError::GroupNotFound)
    }

    /// Registers the working tree at `path` (already resolved and normalized).
    pub fn add_repo(&mut self, path: PathBuf) -> Added {
        if let Some(existing) = self.repos.iter().find(|repo| repo.path == path) {
            return Added::Existing(existing.id.clone());
        }
        let id = RepoId::generate();
        self.repos.push(RepoEntry {
            id: id.clone(),
            path,
            custom_name: None,
            group_id: None,
            favorite: false,
            mainline: None,
        });
        Added::New(id)
    }

    pub fn remove_repo(&mut self, id: &RepoId) -> Result<(), WorkspaceError> {
        self.repo(id)?;
        self.repos.retain(|repo| &repo.id != id);
        if self.active_repo_id.as_ref() == Some(id) {
            self.active_repo_id = None;
        }
        Ok(())
    }

    /// Points a repository at a new location, e.g. after it was moved.
    pub fn relocate_repo(&mut self, id: &RepoId, path: PathBuf) -> Result<(), WorkspaceError> {
        if self
            .repos
            .iter()
            .any(|repo| &repo.id != id && repo.path == path)
        {
            return Err(WorkspaceError::AlreadyRegistered(path));
        }
        self.repo_mut(id)?.path = path;
        Ok(())
    }

    /// Sets the display name; `None` or a blank name restores the folder name.
    pub fn rename_repo(&mut self, id: &RepoId, name: Option<&str>) -> Result<(), WorkspaceError> {
        let name = name.map(str::trim).filter(|name| !name.is_empty());
        self.repo_mut(id)?.custom_name = name.map(str::to_owned);
        Ok(())
    }

    /// Sets the ref branches are compared with; `None` or a blank value
    /// restores the automatic choice ([`DEFAULT_MAINLINE`], else
    /// [`FALLBACK_MAINLINE`], else [`FINAL_FALLBACK_MAINLINE`]). Choosing the
    /// default explicitly keeps it even where it does not exist.
    pub fn set_mainline(
        &mut self,
        id: &RepoId,
        mainline: Option<&str>,
    ) -> Result<(), WorkspaceError> {
        let mainline = mainline.map(str::trim).filter(|m| !m.is_empty());
        self.repo_mut(id)?.mainline = mainline.map(str::to_owned);
        Ok(())
    }

    pub fn set_fetch_interval(&mut self, minutes: u32) {
        self.settings.fetch_interval_minutes = minutes;
    }

    /// Sets the terminal font size, kept within [`TERMINAL_FONT_SIZES`].
    pub fn set_terminal_font_size(&mut self, size: u16) {
        self.settings.terminal_font_size =
            size.clamp(*TERMINAL_FONT_SIZES.start(), *TERMINAL_FONT_SIZES.end());
    }

    pub fn set_sound_muted(&mut self, muted: bool) {
        self.settings.sound_muted = muted;
    }

    pub fn set_mute_when_watching(&mut self, mute: bool) {
        self.settings.mute_when_watching = mute;
    }

    pub fn set_favorite(&mut self, id: &RepoId, favorite: bool) -> Result<(), WorkspaceError> {
        self.repo_mut(id)?.favorite = favorite;
        Ok(())
    }

    pub fn set_repo_group(
        &mut self,
        id: &RepoId,
        group_id: Option<GroupId>,
    ) -> Result<(), WorkspaceError> {
        if let Some(group_id) = &group_id {
            self.group_mut(group_id)?;
        }
        self.repo_mut(id)?.group_id = group_id;
        Ok(())
    }

    pub fn set_active_repo(&mut self, id: Option<&RepoId>) -> Result<(), WorkspaceError> {
        if let Some(id) = id {
            self.repo(id)?;
        }
        self.active_repo_id = id.cloned();
        Ok(())
    }

    pub fn create_group(&mut self, name: &str) -> Result<GroupId, WorkspaceError> {
        let name = self.validate_group_name(name, None)?;
        let id = GroupId::generate();
        self.groups.push(Group {
            id: id.clone(),
            name,
            collapsed: false,
        });
        Ok(id)
    }

    pub fn rename_group(&mut self, id: &GroupId, name: &str) -> Result<(), WorkspaceError> {
        let name = self.validate_group_name(name, Some(id))?;
        self.group_mut(id)?.name = name;
        Ok(())
    }

    pub fn set_group_collapsed(
        &mut self,
        id: &GroupId,
        collapsed: bool,
    ) -> Result<(), WorkspaceError> {
        self.group_mut(id)?.collapsed = collapsed;
        Ok(())
    }

    /// Deletes a group. Its repositories become ungrouped.
    pub fn delete_group(&mut self, id: &GroupId) -> Result<(), WorkspaceError> {
        self.group_mut(id)?;
        self.groups.retain(|group| &group.id != id);
        for repo in &mut self.repos {
            if repo.group_id.as_ref() == Some(id) {
                repo.group_id = None;
            }
        }
        Ok(())
    }

    fn validate_group_name(
        &self,
        name: &str,
        renaming: Option<&GroupId>,
    ) -> Result<String, WorkspaceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(WorkspaceError::EmptyName);
        }
        let taken = self.groups.iter().any(|group| {
            Some(&group.id) != renaming && group.name.to_lowercase() == name.to_lowercase()
        });
        if taken {
            return Err(WorkspaceError::DuplicateGroup(name.to_owned()));
        }
        Ok(name.to_owned())
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with_repo() -> (Registry, RepoId) {
        let mut registry = Registry::default();
        let id = registry.add_repo(PathBuf::from("/code/api")).id().clone();
        (registry, id)
    }

    #[test]
    fn adding_the_same_path_twice_returns_the_existing_entry() {
        let (mut registry, id) = registry_with_repo();
        assert_eq!(
            registry.add_repo(PathBuf::from("/code/api")),
            Added::Existing(id)
        );
        assert_eq!(registry.repos.len(), 1);
    }

    #[test]
    fn display_name_defaults_to_the_folder_name() {
        let (mut registry, id) = registry_with_repo();
        assert_eq!(registry.repo(&id).unwrap().display_name(), "api");

        registry.rename_repo(&id, Some("  Backend  ")).unwrap();
        assert_eq!(registry.repo(&id).unwrap().display_name(), "Backend");

        registry.rename_repo(&id, Some("   ")).unwrap();
        assert_eq!(registry.repo(&id).unwrap().display_name(), "api");
    }

    #[test]
    fn mainline_defaults_and_resets() {
        let (mut registry, id) = registry_with_repo();
        assert_eq!(registry.repo(&id).unwrap().mainline(), None);
        registry.set_mainline(&id, Some(" origin/main ")).unwrap();
        assert_eq!(registry.repo(&id).unwrap().mainline(), Some("origin/main"));
        registry.set_mainline(&id, Some("")).unwrap();
        assert_eq!(registry.repo(&id).unwrap().mainline(), None);
        registry
            .set_mainline(&id, Some("origin/development"))
            .unwrap();
        assert_eq!(
            registry.repo(&id).unwrap().mainline(),
            Some("origin/development")
        );
    }

    #[test]
    fn removing_the_active_repo_clears_the_selection() {
        let (mut registry, id) = registry_with_repo();
        registry.set_active_repo(Some(&id)).unwrap();
        registry.remove_repo(&id).unwrap();
        assert_eq!(registry.repos, []);
        assert_eq!(registry.active_repo_id, None);
    }

    #[test]
    fn group_names_are_trimmed_and_unique_ignoring_case() {
        let mut registry = Registry::default();
        let work = registry.create_group(" Work ").unwrap();
        assert_eq!(registry.groups[0].name, "Work");
        assert!(matches!(
            registry.create_group("work"),
            Err(WorkspaceError::DuplicateGroup(_))
        ));
        assert!(matches!(
            registry.create_group(" "),
            Err(WorkspaceError::EmptyName)
        ));
        // Renaming a group to its own name (in another case) is fine.
        registry.rename_group(&work, "WORK").unwrap();
    }

    #[test]
    fn deleting_a_group_ungroups_its_repos() {
        let (mut registry, id) = registry_with_repo();
        let group = registry.create_group("Work").unwrap();
        registry.set_repo_group(&id, Some(group.clone())).unwrap();

        registry.delete_group(&group).unwrap();
        assert_eq!(registry.groups, []);
        assert_eq!(registry.repo(&id).unwrap().group_id, None);
    }

    #[test]
    fn assigning_an_unknown_group_fails_without_changes() {
        let (mut registry, id) = registry_with_repo();
        let before = registry.clone();
        let unknown = GroupId("nope".into());
        assert!(matches!(
            registry.set_repo_group(&id, Some(unknown)),
            Err(WorkspaceError::GroupNotFound)
        ));
        assert_eq!(registry, before);
    }

    #[test]
    fn relocating_onto_another_repo_is_rejected() {
        let (mut registry, api) = registry_with_repo();
        registry.add_repo(PathBuf::from("/code/web"));
        assert!(matches!(
            registry.relocate_repo(&api, PathBuf::from("/code/web")),
            Err(WorkspaceError::AlreadyRegistered(_))
        ));
        registry
            .relocate_repo(&api, PathBuf::from("/moved/api"))
            .unwrap();
        assert_eq!(
            registry.repo(&api).unwrap().path,
            PathBuf::from("/moved/api")
        );
    }
}
