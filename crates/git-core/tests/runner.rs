//! Integration tests against the real git installation.

mod support;

use git_core::{GitError, GitVersion};
use support::git;

#[tokio::test]
async fn discovers_a_supported_git() {
    let git = git().await;
    assert!(git.version() >= GitVersion::MIN_SUPPORTED);
    assert!(git.program().is_file());
}

#[tokio::test]
async fn runs_commands_in_a_working_directory() {
    let git = git().await;
    let repo = tempfile::tempdir().unwrap();
    git.runner()
        .command(["init", "--quiet"])
        .current_dir(repo.path())
        .output()
        .await
        .unwrap();

    let output = git
        .runner()
        .command(["rev-parse", "--is-inside-work-tree"])
        .current_dir(repo.path())
        .output()
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "true");
}

#[tokio::test]
async fn reports_failures_with_stderr() {
    let git = git().await;
    let not_a_repo = tempfile::tempdir().unwrap();

    let error = git
        .runner()
        .command(["rev-parse", "HEAD"])
        .current_dir(not_a_repo.path())
        .output()
        .await
        .unwrap_err();
    match error {
        GitError::Failed {
            command,
            code,
            stderr,
        } => {
            assert_eq!(command, "rev-parse HEAD");
            assert_eq!(code, Some(128));
            assert_ne!(stderr, "");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn accepts_configured_success_codes() {
    let git = git().await;
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
    tokio::fs::write(&a, "one\n").await.unwrap();
    tokio::fs::write(&b, "two\n").await.unwrap();

    // `git diff --no-index` exits with 1 when the files differ.
    let output = git
        .runner()
        .command(["diff", "--no-index", "--"])
        .args([&a, &b])
        .success_codes(&[0, 1])
        .output()
        .await
        .unwrap();
    assert_eq!(output.exit_code, 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+two"));
}
