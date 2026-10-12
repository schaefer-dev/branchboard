use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use git_core::{DiscoveryOptions, Git, Head};
use std::time::Duration;

use git_core::ChangeKind;
use workspace::{
    Added, EventSink, StatusUpdate, SummaryState, SummaryUpdate, Workspace, WorkspaceError,
};

async fn git() -> Git {
    Git::discover(DiscoveryOptions::default()).await.unwrap()
}

async fn init_repo(git: &Git, dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    git.runner()
        .command(["init", "--quiet", "--initial-branch=main"])
        .current_dir(dir)
        .output()
        .await
        .unwrap();
    dir.canonicalize().unwrap()
}

#[derive(Default)]
struct Collect {
    summaries: Mutex<Vec<SummaryUpdate>>,
    statuses: Mutex<Vec<StatusUpdate>>,
}

impl EventSink for Collect {
    fn summary(&self, update: SummaryUpdate) {
        self.summaries.lock().unwrap().push(update);
    }

    fn status(&self, update: StatusUpdate) {
        self.statuses.lock().unwrap().push(update);
    }
}

fn open(file: PathBuf) -> (Workspace, Arc<Collect>) {
    let events = Arc::new(Collect::default());
    let sink: Arc<dyn EventSink> = events.clone();
    (Workspace::open(file, sink), events)
}

async fn commit_all(git: &Git, repo: &Path) {
    git.runner()
        .command([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-am",
            "commit",
        ])
        .current_dir(repo)
        .output()
        .await
        .unwrap();
}

#[tokio::test]
async fn adds_repos_once_and_persists_them() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let repo = init_repo(&git, &root.path().join("api")).await;
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let file = root.path().join("state").join("workspace.json");

    let (workspace, _) = open(file.clone());
    let (added, snapshot) = workspace.add_repo(&git, &repo.join("src")).await.unwrap();
    assert!(matches!(added, Added::New(_)));
    assert_eq!(snapshot.repos.len(), 1);
    assert_eq!(snapshot.repos[0].name, "api");
    assert_eq!(snapshot.active_repo_id.as_ref(), Some(added.id()));

    let (again, _) = workspace.add_repo(&git, &repo).await.unwrap();
    assert_eq!(again, Added::Existing(added.id().clone()));

    let reopened = open(file).0.snapshot().await;
    assert_eq!(reopened.repos.len(), 1);
    assert_eq!(reopened.active_repo_id.as_ref(), Some(added.id()));
}

#[tokio::test]
async fn rejects_folders_that_are_not_repositories() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let (workspace, _) = open(root.path().join("workspace.json"));

    let error = workspace.add_repo(&git, root.path()).await.unwrap_err();
    assert!(matches!(error, WorkspaceError::Git(_)), "{error:?}");
    assert!(workspace.snapshot().await.repos.is_empty());
}

#[tokio::test]
async fn publishes_summaries_and_detects_missing_repos() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let api = init_repo(&git, &root.path().join("api")).await;
    let web = init_repo(&git, &root.path().join("web")).await;
    let (workspace, events) = open(root.path().join("workspace.json"));
    let (api_id, _) = workspace.add_repo(&git, &api).await.unwrap();
    let (web_id, _) = workspace.add_repo(&git, &web).await.unwrap();
    std::fs::write(api.join("new.txt"), "x").unwrap();
    std::fs::remove_dir_all(&web).unwrap();

    workspace.refresh_summaries(&git, None).await;

    let updates = events.summaries.lock().unwrap().clone();
    assert_eq!(updates.len(), 2);
    let state_of = |id| &updates.iter().find(|u| &u.repo_id == id).unwrap().state;
    match state_of(api_id.id()) {
        SummaryState::Ready { summary } => {
            assert_eq!(
                summary.head,
                Head::Unborn {
                    name: "main".into()
                }
            );
            assert!(summary.dirty);
        }
        other => panic!("unexpected state {other:?}"),
    }
    assert!(matches!(state_of(web_id.id()), SummaryState::Missing));
    assert!(
        workspace
            .snapshot()
            .await
            .repos
            .iter()
            .any(|repo| repo.missing)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stages_unstages_and_discards_through_the_workspace() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let repo = init_repo(&git, &root.path().join("api")).await;
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    std::fs::write(repo.join("b.txt"), "b\n").unwrap();
    git.stage(&repo, &["a.txt".into(), "b.txt".into()])
        .await
        .unwrap();
    commit_all(&git, &repo).await;
    std::fs::write(repo.join("a.txt"), "changed\n").unwrap();
    std::fs::write(repo.join("b.txt"), "changed\n").unwrap();

    let (workspace, events) = open(root.path().join("workspace.json"));
    let (added, _) = workspace.add_repo(&git, &repo).await.unwrap();
    let id = added.id();

    let status = workspace.status(&git, id).await.unwrap();
    assert_eq!(status.unstaged.len(), 2);

    let staged = workspace.stage(&git, id, &["a.txt".into()]).await.unwrap();
    assert_eq!(staged.staged.len(), 1);
    let unstaged = workspace
        .unstage(&git, id, &["a.txt".into()])
        .await
        .unwrap();
    assert_eq!(unstaged.staged, vec![]);

    let discarded = workspace
        .discard(&git, id, &["b.txt".into()])
        .await
        .unwrap();
    assert_eq!(discarded.unstaged.len(), 1);
    assert_eq!(discarded.unstaged[0].kind, ChangeKind::Modified);
    assert_eq!(std::fs::read_to_string(repo.join("b.txt")).unwrap(), "b\n");

    // Every change was published, ending with the current status.
    let last = events.statuses.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.status, discarded);
}

#[tokio::test(flavor = "multi_thread")]
async fn publishes_status_changes_detected_by_the_watcher() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let repo = init_repo(&git, &root.path().join("api")).await;
    let (workspace, events) = open(root.path().join("workspace.json"));
    let (added, _) = workspace.add_repo(&git, &repo).await.unwrap();
    let initial = workspace.status(&git, added.id()).await.unwrap();
    assert!(!initial.is_dirty());

    std::fs::write(repo.join("new.txt"), "x").unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let dirty = events
            .statuses
            .lock()
            .unwrap()
            .iter()
            .any(|update| update.status.is_dirty());
        if dirty {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "no status event");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
