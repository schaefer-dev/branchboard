mod support;

use support::TestRepo;

/// A repository whose commits made through [`git_core::Git`] work without
/// the host's identity or signing settings.
async fn repo() -> TestRepo {
    let repo = TestRepo::init().await;
    repo.run(&["config", "--local", "user.name", "Test"]).await;
    repo.run(&["config", "--local", "user.email", "test@example.com"])
        .await;
    repo.run(&["config", "--local", "commit.gpgsign", "false"])
        .await;
    repo
}

#[tokio::test]
async fn commits_amends_and_undoes() {
    let repo = repo().await;
    let path = repo.path();
    assert_eq!(repo.git.head_commit(&path).await.unwrap(), None);

    repo.write("a.txt", "one\n");
    repo.git.stage(&path, &["a.txt".to_owned()]).await.unwrap();
    repo.git
        .commit(&path, "Add a\n\nWith # a body line.\n\n", false)
        .await
        .unwrap();
    let first = repo.git.head_commit(&path).await.unwrap().unwrap();
    assert_eq!(first.subject, "Add a");
    assert_eq!(first.message, "Add a\n\nWith # a body line.");
    assert!(!first.has_parent);
    assert!(!first.pushed);
    assert!(first.oid.starts_with(&first.short_oid));

    repo.write("a.txt", "two\n");
    repo.git.stage(&path, &["a.txt".to_owned()]).await.unwrap();
    repo.git.commit(&path, "Change a", false).await.unwrap();
    let second = repo.git.head_commit(&path).await.unwrap().unwrap();
    assert!(second.has_parent);

    repo.git
        .commit(&path, "Change a again", true)
        .await
        .unwrap();
    let amended = repo.git.head_commit(&path).await.unwrap().unwrap();
    assert_eq!(amended.subject, "Change a again");
    assert_ne!(amended.oid, second.oid);

    repo.git.undo_last_commit(&path).await.unwrap();
    let head = repo.git.head_commit(&path).await.unwrap().unwrap();
    assert_eq!(head.oid, first.oid);
    let status = repo.git.status(&path).await.unwrap();
    assert_eq!(status.staged.len(), 1);
    assert_eq!(status.head_oid.as_deref(), Some(first.oid.as_str()));
}

#[tokio::test]
async fn knows_when_head_is_on_a_remote() {
    let remote = TestRepo::init().await;
    let repo = repo().await;
    repo.commit_file("a.txt", "one\n").await;
    let url = remote.path();
    repo.run(&["remote", "add", "origin", url.to_str().unwrap()])
        .await;
    // The remote has `main` checked out, so publish under another name.
    repo.run(&["push", "--quiet", "origin", "main:published"])
        .await;
    repo.run(&["fetch", "--quiet", "origin"]).await;
    assert!(
        repo.git
            .head_commit(&repo.path())
            .await
            .unwrap()
            .unwrap()
            .pushed
    );

    repo.commit_file("b.txt", "two\n").await;
    assert!(
        !repo
            .git
            .head_commit(&repo.path())
            .await
            .unwrap()
            .unwrap()
            .pushed
    );
}

#[cfg(unix)]
#[tokio::test]
async fn reports_a_failing_hook() {
    use std::os::unix::fs::PermissionsExt;

    let repo = repo().await;
    let hook = repo.path().join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho 'lint: 2 problems'\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    repo.write("a.txt", "one\n");
    repo.git
        .stage(&repo.path(), &["a.txt".to_owned()])
        .await
        .unwrap();

    let error = repo
        .git
        .commit(&repo.path(), "Add a", false)
        .await
        .unwrap_err();
    let git_core::GitError::Failed { stderr, .. } = error else {
        panic!("unexpected error {error:?}");
    };
    // Git sends hook output to stderr.
    assert!(stderr.contains("lint: 2 problems"), "{stderr}");
    assert_eq!(repo.git.head_commit(&repo.path()).await.unwrap(), None);
}
