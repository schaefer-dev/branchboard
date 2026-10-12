mod support;

use git_core::{ChangeKind, DiffArea, DiffLineKind, DiffRequest, FileChange, StatusSnapshot};
use support::TestRepo;

fn listed(changes: &[FileChange]) -> Vec<(&str, ChangeKind)> {
    changes.iter().map(|c| (c.path.as_str(), c.kind)).collect()
}

fn request(change: &FileChange, area: DiffArea) -> DiffRequest {
    DiffRequest {
        path: change.path.clone(),
        orig_path: change.orig_path.clone(),
        area,
        kind: change.kind,
        full: false,
    }
}

async fn status(repo: &TestRepo) -> StatusSnapshot {
    repo.git.status(&repo.path()).await.unwrap()
}

#[tokio::test]
async fn stages_and_unstages_files_including_special_names() {
    let repo = TestRepo::init().await;
    repo.commit_file("tracked.txt", "one\n").await;
    // `*` must be taken literally, not as a glob matching both files.
    repo.write("star*.txt", "a\n");
    repo.write("starfish.txt", "b\n");
    repo.write("ordner mit ü/datei.txt", "c\n");
    repo.write("tracked.txt", "two\n");

    let before = status(&repo).await;
    assert_eq!(before.staged, vec![]);
    assert_eq!(before.unstaged.len(), 4);

    let paths = [
        "star*.txt".to_owned(),
        "ordner mit ü/datei.txt".to_owned(),
        "tracked.txt".to_owned(),
    ];
    repo.git.stage(&repo.path(), &paths).await.unwrap();
    let staged = status(&repo).await;
    assert_eq!(
        listed(&staged.staged),
        vec![
            ("ordner mit ü/datei.txt", ChangeKind::Added),
            ("star*.txt", ChangeKind::Added),
            ("tracked.txt", ChangeKind::Modified),
        ]
    );
    assert_eq!(
        listed(&staged.unstaged),
        vec![("starfish.txt", ChangeKind::Untracked)]
    );

    repo.git
        .unstage(&repo.path(), &["star*.txt".to_owned()], false)
        .await
        .unwrap();
    let after = status(&repo).await;
    assert_eq!(after.staged.len(), 2);
    assert!(after.find_unstaged("star*.txt").is_some());
}

#[tokio::test]
async fn empty_path_lists_change_nothing() {
    let repo = TestRepo::init().await;
    repo.write("file.txt", "x\n");
    repo.git.stage(&repo.path(), &[]).await.unwrap();
    assert_eq!(status(&repo).await.staged, vec![]);
}

#[tokio::test]
async fn unstages_in_a_repository_without_commits() {
    let repo = TestRepo::init().await;
    repo.write("first.txt", "x\n");
    repo.git
        .stage(&repo.path(), &["first.txt".to_owned()])
        .await
        .unwrap();
    assert_eq!(status(&repo).await.staged.len(), 1);

    repo.git
        .unstage(&repo.path(), &["first.txt".to_owned()], true)
        .await
        .unwrap();
    let after = status(&repo).await;
    assert_eq!(after.staged, vec![]);
    assert_eq!(
        listed(&after.unstaged),
        vec![("first.txt", ChangeKind::Untracked)]
    );
}

#[tokio::test]
async fn unstaging_a_rename_needs_both_paths() {
    let repo = TestRepo::init().await;
    repo.commit_file("old.txt", "content\n").await;
    repo.run(&["mv", "old.txt", "new.txt"]).await;

    let renamed = status(&repo).await;
    let change = renamed.find_staged("new.txt").unwrap();
    assert_eq!(change.kind, ChangeKind::Renamed);
    assert_eq!(change.orig_path.as_deref(), Some("old.txt"));

    repo.git
        .unstage(
            &repo.path(),
            &["new.txt".to_owned(), "old.txt".to_owned()],
            false,
        )
        .await
        .unwrap();
    let after = status(&repo).await;
    assert_eq!(after.staged, vec![]);
    assert_eq!(
        listed(&after.unstaged),
        vec![
            ("old.txt", ChangeKind::Deleted),
            ("new.txt", ChangeKind::Untracked)
        ]
    );
}

#[tokio::test]
async fn restores_tracked_files_from_the_index() {
    let repo = TestRepo::init().await;
    repo.commit_file("file.txt", "original\n").await;
    repo.write("file.txt", "edited\n");

    repo.git
        .restore_worktree(&repo.path(), &["file.txt".to_owned()])
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap(),
        "original\n"
    );
    assert!(!status(&repo).await.is_dirty());
}

#[tokio::test]
async fn diffs_staged_unstaged_and_untracked_files() {
    let repo = TestRepo::init().await;
    repo.commit_file("file.txt", "a\nb\nc\n").await;
    repo.write("file.txt", "a\nB\nc\n");
    repo.git
        .stage(&repo.path(), &["file.txt".to_owned()])
        .await
        .unwrap();
    repo.write("file.txt", "a\nB\nc\nd\n");
    repo.write("new.txt", "hello\n");

    let snapshot = status(&repo).await;

    let staged = repo
        .git
        .diff(
            &repo.path(),
            &request(snapshot.find_staged("file.txt").unwrap(), DiffArea::Staged),
        )
        .await
        .unwrap();
    assert_eq!((staged.additions, staged.deletions), (1, 1));

    let unstaged = repo
        .git
        .diff(
            &repo.path(),
            &request(
                snapshot.find_unstaged("file.txt").unwrap(),
                DiffArea::Unstaged,
            ),
        )
        .await
        .unwrap();
    assert_eq!((unstaged.additions, unstaged.deletions), (1, 0));
    let added = unstaged.hunks[0]
        .lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Added)
        .unwrap();
    assert_eq!((added.text.as_str(), added.new_line), ("d", Some(4)));

    let untracked = repo
        .git
        .diff(
            &repo.path(),
            &request(
                snapshot.find_unstaged("new.txt").unwrap(),
                DiffArea::Unstaged,
            ),
        )
        .await
        .unwrap();
    assert_eq!(untracked.additions, 1);
    assert_eq!(untracked.notes, vec!["new file mode 100644"]);
}
