use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use git_core::{DiscoveryOptions, Git, Head};
use workspace::{Added, SummarySink, SummaryState, SummaryUpdate, Workspace, WorkspaceError};

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
struct Collect(Mutex<Vec<SummaryUpdate>>);

impl SummarySink for Collect {
    fn publish(&self, update: SummaryUpdate) {
        self.0.lock().unwrap().push(update);
    }
}

#[tokio::test]
async fn adds_repos_once_and_persists_them() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let repo = init_repo(&git, &root.path().join("api")).await;
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let file = root.path().join("state").join("workspace.json");

    let workspace = Workspace::open(file.clone());
    let (added, snapshot) = workspace.add_repo(&git, &repo.join("src")).await.unwrap();
    assert!(matches!(added, Added::New(_)));
    assert_eq!(snapshot.repos.len(), 1);
    assert_eq!(snapshot.repos[0].name, "api");
    assert_eq!(snapshot.active_repo_id.as_ref(), Some(added.id()));

    let (again, _) = workspace.add_repo(&git, &repo).await.unwrap();
    assert_eq!(again, Added::Existing(added.id().clone()));

    let reopened = Workspace::open(file).snapshot().await;
    assert_eq!(reopened.repos.len(), 1);
    assert_eq!(reopened.active_repo_id.as_ref(), Some(added.id()));
}

#[tokio::test]
async fn rejects_folders_that_are_not_repositories() {
    let git = git().await;
    let root = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(root.path().join("workspace.json"));

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
    let workspace = Workspace::open(root.path().join("workspace.json"));
    let (api_id, _) = workspace.add_repo(&git, &api).await.unwrap();
    let (web_id, _) = workspace.add_repo(&git, &web).await.unwrap();
    std::fs::write(api.join("new.txt"), "x").unwrap();
    std::fs::remove_dir_all(&web).unwrap();

    let collect = Arc::new(Collect::default());
    let sink: Arc<dyn SummarySink> = collect.clone();
    workspace.refresh_summaries(&git, None, &sink).await;

    let updates = collect.0.lock().unwrap().clone();
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
