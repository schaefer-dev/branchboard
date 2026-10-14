mod support;

use git_core::{ChangeKind, ReviewDiffRequest, ReviewFile, ReviewTarget};
use support::TestRepo;

fn listed(files: &[ReviewFile]) -> Vec<(&str, ChangeKind)> {
    files.iter().map(|f| (f.path.as_str(), f.kind)).collect()
}

async fn head(repo: &TestRepo) -> String {
    repo.run(&["rev-parse", "HEAD"]).await.trim().to_owned()
}

/// `main` with a feature branch that renames, edits, adds a binary file and
/// deletes a file; `main` moves on after the branch left it.
async fn branch_repo() -> TestRepo {
    let repo = TestRepo::init().await;
    repo.write("keep.txt", "1\n2\n3\n");
    repo.write("old name.txt", "same content\nfor the rename\n");
    repo.write("gone.txt", "bye\n");
    repo.run(&["add", "."]).await;
    repo.run(&["commit", "--quiet", "-m", "base"]).await;

    repo.run(&["switch", "--quiet", "-c", "feature"]).await;
    repo.run(&["mv", "old name.txt", "new name.txt"]).await;
    repo.write("keep.txt", "1\ntwo\n3\n");
    repo.write("logo.png", "\u{0}\u{1}binary");
    repo.run(&["rm", "--quiet", "gone.txt"]).await;
    repo.run(&["add", "."]).await;
    repo.run(&["commit", "--quiet", "-m", "feature work"]).await;
    repo.commit_file("keep.txt", "1\ntwo\nthree\n").await;

    repo.run(&["switch", "--quiet", "main"]).await;
    repo.commit_file("main-only.txt", "later\n").await;
    repo
}

#[tokio::test]
async fn reviews_a_branch_from_its_merge_base() {
    let repo = branch_repo().await;
    let path = repo.path();
    let target = ReviewTarget::Branch {
        name: "feature".into(),
    };
    let range = repo.git.review_range(&path, &target, "main").await.unwrap();
    let base = repo.run(&["merge-base", "main", "feature"]).await;
    assert_eq!(range.from, base.trim());
    assert_eq!(range.to, repo.run(&["rev-parse", "feature"]).await.trim());

    let files = repo.git.review_files(&path, &range).await.unwrap();
    // Changes on `main` after the branch left are not part of it.
    assert_eq!(
        listed(&files),
        vec![
            ("gone.txt", ChangeKind::Deleted),
            ("keep.txt", ChangeKind::Modified),
            ("logo.png", ChangeKind::Added),
            ("new name.txt", ChangeKind::Renamed),
        ]
    );
    let keep = &files[1];
    assert_eq!((keep.additions, keep.deletions), (Some(2), Some(2)));
    assert_eq!(files[2].additions, None);
    assert_eq!(files[3].orig_path.as_deref(), Some("old name.txt"));

    let request = ReviewDiffRequest {
        range: range.clone(),
        path: "keep.txt".into(),
        orig_path: None,
        full: false,
    };
    let diff = repo.git.review_diff(&path, &request).await.unwrap();
    assert_eq!((diff.additions, diff.deletions), (2, 2));
    let sources = repo.git.review_sources(&path, &request).await.unwrap();
    assert_eq!(sources.old.as_deref(), Some("1\n2\n3\n"));
    assert_eq!(sources.new.as_deref(), Some("1\ntwo\nthree\n"));

    let rename = ReviewDiffRequest {
        range,
        path: "new name.txt".into(),
        orig_path: Some("old name.txt".into()),
        full: false,
    };
    let diff = repo.git.review_diff(&path, &rename).await.unwrap();
    assert!(
        diff.notes
            .iter()
            .any(|note| note == "rename from old name.txt")
    );
    assert_eq!(diff.hunks, vec![]);
}

#[tokio::test]
async fn reviews_single_commits_including_the_first() {
    let repo = branch_repo().await;
    let path = repo.path();
    repo.run(&["switch", "--quiet", "feature"]).await;
    let tip = head(&repo).await;

    let range = repo
        .git
        .review_range(&path, &ReviewTarget::Commit { oid: tip.clone() }, "main")
        .await
        .unwrap();
    let files = repo.git.review_files(&path, &range).await.unwrap();
    assert_eq!(listed(&files), vec![("keep.txt", ChangeKind::Modified)]);

    let root = repo.run(&["rev-list", "--max-parents=0", "HEAD"]).await;
    let range = repo
        .git
        .review_range(
            &path,
            &ReviewTarget::Commit {
                oid: root.trim().into(),
            },
            "main",
        )
        .await
        .unwrap();
    let files = repo.git.review_files(&path, &range).await.unwrap();
    assert_eq!(
        listed(&files),
        vec![
            ("gone.txt", ChangeKind::Added),
            ("keep.txt", ChangeKind::Added),
            ("old name.txt", ChangeKind::Added),
        ]
    );
    let request = ReviewDiffRequest {
        range,
        path: "keep.txt".into(),
        orig_path: None,
        full: false,
    };
    let sources = repo.git.review_sources(&path, &request).await.unwrap();
    assert_eq!(sources.old, None);
    assert_eq!(sources.new.as_deref(), Some("1\n2\n3\n"));
}

#[tokio::test]
async fn compares_merge_commits_with_their_first_parent() {
    let repo = branch_repo().await;
    let path = repo.path();
    repo.run(&[
        "merge",
        "--quiet",
        "--no-ff",
        "-m",
        "merge feature",
        "feature",
    ])
    .await;
    let merge = head(&repo).await;
    let range = repo
        .git
        .review_range(&path, &ReviewTarget::Commit { oid: merge }, "main")
        .await
        .unwrap();
    let files = repo.git.review_files(&path, &range).await.unwrap();
    // What the merge brought into `main`.
    assert_eq!(files.len(), 4);
    assert!(!files.iter().any(|f| f.path == "main-only.txt"));
}

#[tokio::test]
async fn rejects_unknown_targets() {
    let repo = branch_repo().await;
    let target = ReviewTarget::Branch {
        name: "nope".into(),
    };
    assert!(
        repo.git
            .review_range(&repo.path(), &target, "main")
            .await
            .is_err()
    );
    let target = ReviewTarget::Commit {
        oid: "--output=/tmp/x".into(),
    };
    assert!(
        repo.git
            .review_range(&repo.path(), &target, "main")
            .await
            .is_err()
    );
}
