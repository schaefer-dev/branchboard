mod support;

use git_core::{AheadBehind, Branch, GitError, UpstreamState};
use support::TestRepo;

fn branch<'a>(branches: &'a [Branch], name: &str) -> &'a Branch {
    branches.iter().find(|b| b.name == name).unwrap()
}

/// A repository whose `development` is pushed to a bare `origin`, with
/// `feature` two commits ahead of it.
async fn with_remote() -> (TestRepo, tempfile::TempDir) {
    let remote = tempfile::tempdir().unwrap();
    let repo = TestRepo::init().await;
    repo.git
        .runner()
        .command(["init", "--quiet", "--bare", "--initial-branch=development"])
        .current_dir(remote.path())
        .output()
        .await
        .unwrap();
    repo.run(&["checkout", "--quiet", "-b", "development"])
        .await;
    repo.commit_file("base.txt", "base\n").await;
    let url = remote.path().to_str().unwrap();
    repo.run(&["remote", "add", "origin", url]).await;
    repo.run(&["push", "--quiet", "-u", "origin", "development"])
        .await;

    repo.run(&["checkout", "--quiet", "-b", "feature"]).await;
    repo.commit_file("feature.txt", "one\n").await;
    repo.commit_file("feature.txt", "two\n").await;
    repo.run(&["push", "--quiet", "-u", "origin", "feature"])
        .await;
    repo.commit_file("feature.txt", "three\n").await;
    (repo, remote)
}

#[tokio::test]
async fn counts_against_mainline_and_upstream() {
    let (repo, _remote) = with_remote().await;
    repo.run(&["checkout", "--quiet", "-b", "spike", "development"])
        .await;
    repo.commit_file("spike.txt", "x\n").await;

    let list = repo
        .git
        .branches(&repo.path(), "origin/development")
        .await
        .unwrap();
    assert!(list.mainline_exists);
    let feature = branch(&list.branches, "feature");
    assert_eq!(
        feature.mainline,
        Some(AheadBehind {
            ahead: 3,
            behind: 0
        })
    );
    assert_eq!(
        feature.upstream,
        UpstreamState::Tracking {
            name: "origin/feature".into(),
            counts: AheadBehind {
                ahead: 1,
                behind: 0
            },
        }
    );
    let spike = branch(&list.branches, "spike");
    assert!(spike.current);
    assert_eq!(spike.upstream, UpstreamState::None);
    assert!(!branch(&list.branches, "development").current);
}

#[tokio::test]
async fn reports_gone_upstreams_and_missing_mainlines() {
    let (repo, _remote) = with_remote().await;
    repo.run(&["push", "--quiet", "origin", "--delete", "feature"])
        .await;
    repo.run(&["fetch", "--quiet", "--prune"]).await;

    let list = repo
        .git
        .branches(&repo.path(), "origin/main")
        .await
        .unwrap();
    assert!(!list.mainline_exists);
    assert!(list.branches.iter().all(|b| b.mainline.is_none()));
    assert_eq!(
        branch(&list.branches, "feature").upstream,
        UpstreamState::Gone {
            name: "origin/feature".into()
        }
    );
}

#[tokio::test]
async fn switches_branches_and_refuses_to_overwrite_changes() {
    let (repo, _remote) = with_remote().await;
    repo.git.switch(&repo.path(), "development").await.unwrap();
    let list = repo
        .git
        .branches(&repo.path(), "origin/development")
        .await
        .unwrap();
    assert!(branch(&list.branches, "development").current);

    // `base.txt` differs between the branches only if feature changes it.
    repo.run(&["checkout", "--quiet", "feature"]).await;
    repo.commit_file("base.txt", "feature version\n").await;
    repo.write("base.txt", "uncommitted\n");
    let error = repo
        .git
        .switch(&repo.path(), "development")
        .await
        .unwrap_err();
    assert!(
        matches!(error, GitError::Failed { ref stderr, .. } if !stderr.is_empty()),
        "{error:?}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("base.txt")).unwrap(),
        "uncommitted\n"
    );
}

#[tokio::test]
async fn fetches_and_records_the_time() {
    let (repo, _remote) = with_remote().await;
    let before = repo.git.last_fetched(&repo.path()).await.unwrap();
    assert_eq!(before, None);

    repo.git.fetch(&repo.path()).await.unwrap();
    let after = repo.git.last_fetched(&repo.path()).await.unwrap();
    assert!(after.is_some());
}

#[tokio::test]
async fn lists_commits_between_refs() {
    let (repo, _remote) = with_remote().await;
    let ahead = repo
        .git
        .log(&repo.path(), Some("origin/development"), "feature", 2)
        .await
        .unwrap();
    assert_eq!(ahead.commits.len(), 2);
    assert!(ahead.truncated);
    assert_eq!(ahead.commits[0].subject, "update feature.txt");
    assert_eq!(ahead.commits[0].author, "Test");

    let behind = repo
        .git
        .log(&repo.path(), Some("feature"), "origin/development", 100)
        .await
        .unwrap();
    assert_eq!(behind.commits, vec![]);
    assert!(!behind.truncated);

    let all = repo
        .git
        .log(&repo.path(), None, "feature", 100)
        .await
        .unwrap();
    assert_eq!(all.commits.len(), 4);
}
