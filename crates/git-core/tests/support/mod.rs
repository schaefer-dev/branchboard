//! Helpers for integration tests that run against real git repositories.

#![allow(dead_code, reason = "each test binary uses a different subset")]

use std::path::{Path, PathBuf};

use git_core::{DiscoveryOptions, Git};
use tempfile::TempDir;

pub async fn git() -> Git {
    Git::discover(DiscoveryOptions::default())
        .await
        .expect("git must be installed to run the tests")
}

/// A throwaway repository in a temporary directory.
pub struct TestRepo {
    pub git: Git,
    dir: TempDir,
}

impl TestRepo {
    /// An empty repository whose initial branch is `main`.
    pub async fn init() -> Self {
        let repo = Self {
            git: git().await,
            dir: tempfile::Builder::new()
                .prefix("branchboard test ")
                .tempdir()
                .unwrap(),
        };
        repo.run(&["init", "--quiet", "--initial-branch=main"])
            .await;
        // Keep fixture bytes consistent regardless of the host's Git settings.
        repo.run(&["config", "--local", "core.autocrlf", "false"])
            .await;
        repo.run(&["config", "--local", "core.eol", "lf"]).await;
        repo
    }

    /// Resolves macOS temp-dir symlinks without Windows verbatim prefixes,
    /// which Git would interpret as SSH addresses when used as local remotes.
    pub fn path(&self) -> PathBuf {
        dunce::canonicalize(self.dir.path()).unwrap()
    }

    pub async fn run(&self, args: &[&str]) -> String {
        let output = self
            .git
            .runner()
            .command([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(self.dir.path())
            .output()
            .await
            .unwrap_or_else(|error| panic!("git {args:?} failed: {error:?}"));
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn write(&self, relative: impl AsRef<Path>, contents: &str) {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// Writes a file and commits it.
    pub async fn commit_file(&self, relative: &str, contents: &str) {
        self.write(relative, contents);
        self.run(&["add", "--", relative]).await;
        self.run(&["commit", "--quiet", "-m", &format!("update {relative}")])
            .await;
    }
}
