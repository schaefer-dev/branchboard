mod support;

use git_core::HistoryRequest;
use support::TestRepo;

fn request() -> HistoryRequest {
    HistoryRequest {
        reference: "HEAD".into(),
        query: String::new(),
        tip: None,
        cursor: None,
        limit: 100,
    }
}

#[tokio::test]
async fn pages_are_pinned_complete_and_search_reaches_the_root() {
    let repo = TestRepo::init().await;
    for i in 0..235 {
        let message = if i == 0 {
            "root\n\nLiteral [a.*] body".to_owned()
        } else {
            format!("change {i}")
        };
        repo.run(&["commit", "--quiet", "--allow-empty", "-m", &message])
            .await;
    }
    let mut req = request();
    let first = repo.git.history(&repo.path(), &req).await.unwrap();
    assert_eq!(first.commits.len(), 100);
    let expected = repo.run(&["log", "--topo-order", "--format=%H"]).await;
    repo.run(&["commit", "--quiet", "--allow-empty", "-m", "new tip"])
        .await;
    let mut all = first.commits.clone();
    req.tip = first.tip.clone();
    req.cursor = first.cursor;
    while req.cursor.is_some() {
        let page = repo.git.history(&repo.path(), &req).await.unwrap();
        assert_eq!(page.commits.len(), if all.len() == 100 { 100 } else { 35 });
        all.extend(page.commits);
        req.cursor = page.cursor;
    }
    assert_eq!(
        all.iter().map(|c| c.oid.as_str()).collect::<Vec<_>>(),
        expected.lines().collect::<Vec<_>>()
    );
    req.query = "[A.*]".into();
    let found = repo.git.history(&repo.path(), &req).await.unwrap();
    assert_eq!(found.commits.len(), 1);
    assert_eq!(found.commits[0].subject, "root");
    req.query = all[234].oid[0..9].to_uppercase();
    assert_eq!(
        repo.git.history(&repo.path(), &req).await.unwrap().commits[0].oid,
        all[234].oid
    );
    req.query = "tEsT".into(); // Author matches although no message does.
    assert_eq!(
        repo.git
            .history(&repo.path(), &req)
            .await
            .unwrap()
            .commits
            .len(),
        100
    );
    req.query = "no such author or message".into();
    assert_eq!(
        repo.git.history(&repo.path(), &req).await.unwrap().commits,
        [] as [git_core::CommitSummary; 0]
    );

    req.query.clear();
    req.cursor = repo.git.history(&repo.path(), &req).await.unwrap().cursor;
    req.query = "changed query".into();
    assert!(repo.git.history(&repo.path(), &req).await.is_err());
    req.query.clear();
    req.cursor.as_mut().unwrap().tip = "bad".into();
    assert!(repo.git.history(&repo.path(), &req).await.is_err());
}

#[tokio::test]
async fn handles_unborn_detached_deleted_refs_and_invalid_options() {
    let repo = TestRepo::init().await;
    let mut req = request();
    assert!(
        repo.git
            .history(&repo.path(), &req)
            .await
            .unwrap()
            .tip
            .is_none()
    );
    repo.commit_file("a", "root\n").await;
    let first = repo.git.history(&repo.path(), &req).await.unwrap();
    repo.run(&["switch", "--quiet", "--detach"]).await;
    assert_eq!(
        repo.git.history(&repo.path(), &req).await.unwrap().tip,
        first.tip
    );
    req.reference = "refs/heads/main".into();
    assert_eq!(
        repo.git.history(&repo.path(), &req).await.unwrap().tip,
        first.tip
    );
    repo.run(&["branch", "-D", "main"]).await;
    assert!(repo.git.history(&repo.path(), &req).await.is_err());
    req.tip = first.tip;
    assert_eq!(
        repo.git
            .history(&repo.path(), &req)
            .await
            .unwrap()
            .commits
            .len(),
        1
    );
    req.tip = Some("f".repeat(40));
    assert!(repo.git.history(&repo.path(), &req).await.is_err());
    for reference in ["--all", "refs/tags/a", "refs/heads/../bad", "HEAD~1"] {
        req.reference = reference.into();
        assert!(repo.git.history(&repo.path(), &req).await.is_err());
    }
    req = request();
    req.limit = 101;
    assert!(repo.git.history(&repo.path(), &req).await.is_err());
}

#[tokio::test]
async fn merge_history_matches_git_topological_order_including_both_parents() {
    let repo = TestRepo::init().await;
    repo.commit_file("a", "root").await;
    repo.run(&["switch", "--quiet", "-c", "feature"]).await;
    repo.commit_file("b", "feature").await;
    repo.run(&["switch", "--quiet", "main"]).await;
    repo.commit_file("c", "main").await;
    repo.run(&["merge", "--quiet", "--no-ff", "feature", "-m", "merge"])
        .await;
    let mut req = request();
    req.limit = 1;
    let mut all = Vec::new();
    loop {
        let page = repo.git.history(&repo.path(), &req).await.unwrap();
        all.extend(page.commits.into_iter().map(|c| c.oid));
        req.tip = page.tip;
        req.cursor = page.cursor;
        if req.cursor.is_none() {
            break;
        }
    }
    assert_eq!(all.len(), 4);
    assert_eq!(
        all,
        repo.run(&["log", "--topo-order", "--format=%H"])
            .await
            .lines()
            .collect::<Vec<_>>()
    );
}
