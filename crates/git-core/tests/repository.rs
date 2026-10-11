mod support;

use git_core::{GitError, Head};
use support::TestRepo;

#[tokio::test]
async fn resolves_work_tree_from_a_subfolder() {
    let repo = TestRepo::init().await;
    repo.write("nested/deeper/file.txt", "x");

    let resolved = repo
        .git
        .resolve_work_tree(&repo.path().join("nested/deeper"))
        .await
        .unwrap();
    assert_eq!(resolved.canonicalize().unwrap(), repo.path());
}

#[tokio::test]
async fn rejects_folders_outside_a_repository() {
    let git = support::git().await;
    let dir = tempfile::tempdir().unwrap();

    let error = git.resolve_work_tree(dir.path()).await.unwrap_err();
    assert!(
        matches!(error, GitError::NotARepository { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn rejects_bare_repositories() {
    let git = support::git().await;
    let dir = tempfile::tempdir().unwrap();
    git.runner()
        .command(["init", "--quiet", "--bare"])
        .current_dir(dir.path())
        .output()
        .await
        .unwrap();

    let error = git.resolve_work_tree(dir.path()).await.unwrap_err();
    assert!(
        matches!(error, GitError::BareRepository { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn summarizes_head_and_dirty_state() {
    let repo = TestRepo::init().await;

    let unborn = repo.git.summary(&repo.path()).await.unwrap();
    assert_eq!(
        unborn.head,
        Head::Unborn {
            name: "main".into()
        }
    );
    assert!(!unborn.dirty);

    repo.commit_file("readme.md", "hello\n").await;
    let clean = repo.git.summary(&repo.path()).await.unwrap();
    assert_eq!(
        clean.head,
        Head::Branch {
            name: "main".into()
        }
    );
    assert!(!clean.dirty);

    repo.write("readme.md", "changed\n");
    assert!(repo.git.summary(&repo.path()).await.unwrap().dirty);

    repo.run(&["checkout", "--quiet", "--detach", "HEAD"]).await;
    let detached = repo.git.summary(&repo.path()).await.unwrap();
    assert!(matches!(detached.head, Head::Detached { ref oid } if oid.len() == 7));
}

#[tokio::test]
async fn rejects_files() {
    let repo = TestRepo::init().await;
    repo.write("file.txt", "x");

    let error = repo
        .git
        .resolve_work_tree(&repo.path().join("file.txt"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, GitError::NotARepository { .. }),
        "{error:?}"
    );
}
