//! Contract tests: the real command handlers, invoked through Tauri's IPC
//! layer exactly as the frontend calls them (command names and camelCase
//! arguments from `ui/src/lib/api/commands.ts`), against real repositories.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use branchboard_lib::{AppOptions, build};
use serde_json::{Value, json};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::{App, Manager, WebviewWindow};
use tempfile::TempDir;

struct Harness {
    _app: App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
    dir: TempDir,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let options = AppOptions {
            workspace_file: Some(dir.path().join("workspace.json")),
        };
        // The app's real context, so the real capabilities (ACL) apply.
        let mut app = build(mock_builder(), options)
            .build(tauri::generate_context!(test = true))
            .unwrap();
        // `setup` (state, the configured windows) runs on the first event
        // loop iteration.
        #[allow(deprecated, reason = "a single iteration runs `setup`, not a loop")]
        app.run_iteration(|_, _| {});
        let webview = app.get_webview_window("main").unwrap();
        Self {
            _app: app,
            webview,
            dir,
        }
    }

    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let request = tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            // Test builds run in dev mode, where the dev server is the app's
            // own (trusted) origin.
            url: "http://localhost:1420".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(args),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&self.webview, request).map(|body| body.deserialize().unwrap())
    }

    fn ok(&self, cmd: &str, args: Value) -> Value {
        self.invoke(cmd, args)
            .unwrap_or_else(|error| panic!("{cmd} failed: {error}"))
    }

    /// Creates a repository with deterministic history below the temp dir.
    fn repo(&self, name: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        git(&path, &["init", "--quiet", "--initial-branch=development"]);
        path.canonicalize().unwrap()
    }

    fn add(&self, path: &Path) -> String {
        let result = self.ok("add_repo", json!({ "path": path }));
        result["repoId"].as_str().unwrap().to_owned()
    }
}

fn git(repo: &Path, args: &[&str]) {
    git_at(repo, "2026-01-01T12:00:00Z", args);
}

/// Runs git with author and committer dates fixed to `date`, so commits
/// (and their order by date) are reproducible.
fn git_at(repo: &Path, date: &str, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=Branchboard Tests",
            "-c",
            "user.email=tests@branchboard.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .current_dir(repo)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn write(repo: &Path, relative: &str, contents: impl AsRef<[u8]>) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn paths(changes: &Value) -> Vec<(String, String)> {
    changes
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["path"].as_str().unwrap().to_owned(),
                c["kind"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn reports_app_and_git_info() {
    let harness = Harness::new();
    let app = harness.ok("get_app_info", json!({}));
    assert!(app["version"].is_string());

    let git = harness.ok("get_git_info", json!({}));
    assert!(git["path"].is_string());
    assert!(git["version"]["major"].as_u64().unwrap() >= 2);
}

#[test]
fn manages_repositories_and_groups() {
    let harness = Harness::new();
    let api = harness.repo("api");

    let added = harness.ok("add_repo", json!({ "path": api.join(".") }));
    let repo_id = added["repoId"].as_str().unwrap().to_owned();
    assert_eq!(added["alreadyAdded"], false);
    assert_eq!(added["workspace"]["activeRepoId"], repo_id.as_str());
    assert_eq!(added["workspace"]["repos"][0]["name"], "api");
    assert_eq!(added["workspace"]["repos"][0]["missing"], false);

    let again = harness.ok("add_repo", json!({ "path": api }));
    assert_eq!(again["alreadyAdded"], true);

    let renamed = harness.ok(
        "rename_repo",
        json!({ "repoId": repo_id, "name": "Backend" }),
    );
    assert_eq!(renamed["repos"][0]["name"], "Backend");
    assert_eq!(renamed["repos"][0]["customName"], "Backend");
    let reset = harness.ok("rename_repo", json!({ "repoId": repo_id, "name": null }));
    assert_eq!(reset["repos"][0]["name"], "api");

    let favorite = harness.ok(
        "set_repo_favorite",
        json!({ "repoId": repo_id, "favorite": true }),
    );
    assert_eq!(favorite["repos"][0]["favorite"], true);

    let grouped = harness.ok("create_group", json!({ "name": "Work", "repoId": repo_id }));
    let group_id = grouped["groups"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(grouped["repos"][0]["groupId"], group_id.as_str());

    let ungrouped = harness.ok(
        "set_repo_group",
        json!({ "repoId": repo_id, "groupId": null }),
    );
    assert_eq!(ungrouped["repos"][0]["groupId"], Value::Null);
    harness.ok(
        "set_repo_group",
        json!({ "repoId": repo_id, "groupId": group_id }),
    );

    let renamed_group = harness.ok(
        "rename_group",
        json!({ "groupId": group_id, "name": "Clients" }),
    );
    assert_eq!(renamed_group["groups"][0]["name"], "Clients");
    let collapsed = harness.ok(
        "set_group_collapsed",
        json!({ "groupId": group_id, "collapsed": true }),
    );
    assert_eq!(collapsed["groups"][0]["collapsed"], true);

    let duplicate = harness
        .invoke("create_group", json!({ "name": "clients", "repoId": null }))
        .unwrap_err();
    assert_eq!(duplicate["kind"], "invalidInput");

    let deleted = harness.ok("delete_group", json!({ "groupId": group_id }));
    assert_eq!(deleted["groups"], json!([]));
    assert_eq!(deleted["repos"][0]["groupId"], Value::Null);

    let inactive = harness.ok("set_active_repo", json!({ "repoId": null }));
    assert_eq!(inactive["activeRepoId"], Value::Null);
    harness.ok("set_active_repo", json!({ "repoId": repo_id }));

    let removed = harness.ok("remove_repo", json!({ "repoId": repo_id }));
    assert_eq!(removed["repos"], json!([]));
    assert_eq!(harness.ok("get_workspace", json!({}))["repos"], json!([]));
}

#[test]
fn reports_structured_errors() {
    let harness = Harness::new();
    let error = harness
        .invoke("add_repo", json!({ "path": harness.dir.path() }))
        .unwrap_err();
    assert_eq!(error["kind"], "notARepository");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .ends_with("is not a git repository.")
    );
    assert!(error["detail"].is_string());

    let unknown = harness
        .invoke("remove_repo", json!({ "repoId": "does-not-exist" }))
        .unwrap_err();
    assert_eq!(unknown["kind"], "notFound");
}

#[test]
fn relocates_moved_repositories() {
    let harness = Harness::new();
    let original = harness.repo("before");
    let repo_id = harness.add(&original);
    let moved = harness.dir.path().join("after");
    std::fs::rename(&original, &moved).unwrap();

    let missing = harness.ok("get_workspace", json!({}));
    assert_eq!(missing["repos"][0]["missing"], true);

    let relocated = harness.ok("relocate_repo", json!({ "repoId": repo_id, "path": moved }));
    assert_eq!(relocated["repos"][0]["missing"], false);
    assert_eq!(relocated["repos"][0]["name"], "after");
}

#[test]
fn reads_and_changes_the_working_tree() {
    let harness = Harness::new();
    let repo = harness.repo("api");
    write(&repo, "a.txt", "a\n");
    write(&repo, "b.txt", "b\n");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
    write(&repo, "a.txt", "a changed\n");
    write(&repo, "b.txt", "b changed\n");
    write(&repo, "new.txt", "new\n");
    let repo_id = harness.add(&repo);

    let status = harness.ok("get_status", json!({ "repoId": repo_id }));
    assert_eq!(
        status["head"],
        json!({ "kind": "branch", "name": "development" })
    );
    assert_eq!(status["staged"], json!([]));
    assert_eq!(
        paths(&status["unstaged"]),
        vec![
            ("a.txt".into(), "modified".into()),
            ("b.txt".into(), "modified".into()),
            ("new.txt".into(), "untracked".into()),
        ]
    );

    let diff = harness.ok(
        "get_diff",
        json!({ "repoId": repo_id, "request": {
            "path": "a.txt", "origPath": null, "area": "unstaged", "kind": "modified", "full": false
        }}),
    );
    assert_eq!(
        (diff["additions"].as_u64(), diff["deletions"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(diff["hunks"][0]["lines"][0]["kind"], "removed");
    assert_eq!(diff["hunks"][0]["lines"][1]["newLine"], 1);

    let staged = harness.ok(
        "stage_paths",
        json!({ "repoId": repo_id, "paths": ["a.txt", "new.txt"] }),
    );
    assert_eq!(
        paths(&staged["staged"]),
        vec![
            ("a.txt".into(), "modified".into()),
            ("new.txt".into(), "added".into())
        ]
    );

    let unstaged = harness.ok(
        "unstage_paths",
        json!({ "repoId": repo_id, "paths": ["new.txt"] }),
    );
    assert_eq!(
        paths(&unstaged["staged"]),
        vec![("a.txt".into(), "modified".into())]
    );

    let discarded = harness.ok(
        "discard_paths",
        json!({ "repoId": repo_id, "paths": ["b.txt"] }),
    );
    assert_eq!(
        paths(&discarded["unstaged"]),
        vec![("new.txt".into(), "untracked".into())]
    );
    assert_eq!(std::fs::read_to_string(repo.join("b.txt")).unwrap(), "b\n");

    assert_eq!(harness.ok("refresh_summaries", json!({})), Value::Null);
}

/// A repository with one change of every kind: staged and unstaged edits,
/// a rename, a deletion, untracked text, binary and very large files.
fn fixture_repo(harness: &Harness) -> PathBuf {
    let repo = harness.repo("api");
    let mut login = [
        "use std::collections::HashMap;",
        "",
        "/// Handles login requests.",
        "pub fn login(user: &str, password: &str) -> Result<Session, Error> {",
        "    let users = load_users()?;",
        "    let Some(hash) = users.get(user) else {",
        "        return Err(Error::UnknownUser);",
        "    };",
        "    verify(hash, password)?;",
        "    Ok(Session::new(user))",
        "}",
    ]
    .join("\n");
    login.push('\n');
    for n in 1..=40 {
        writeln!(login, "// line {n}").unwrap();
    }
    write(&repo, "src/handlers/login.rs", &login);
    write(&repo, "config.toml", "[server]\nport = 8080\n");
    write(&repo, "legacy.txt", "old notes\n");
    write(&repo, "README.md", "# api\n");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);

    let edited = login
        .replace(
            "    let users = load_users()?;\n",
            "    let users = load_users().context(\"loading users\")?;\n    tracing::debug!(user, \"login attempt\");\n",
        )
        .replace("Error::UnknownUser)", "Error::UnknownUser(user.to_owned()))")
        .replace("// line 30\n", "// line 30 (changed)\n");
    write(&repo, "src/handlers/login.rs", edited);
    write(&repo, "config.toml", "[server]\nport = 9090\n");
    git(&repo, &["add", "config.toml"]);
    write(
        &repo,
        "config.toml",
        "[server]\nport = 9090\nhost = \"0.0.0.0\"\n",
    );
    git(&repo, &["mv", "legacy.txt", "legacy-notes.txt"]);
    std::fs::remove_file(repo.join("README.md")).unwrap();
    write(&repo, "notes.md", "scratch\n");
    write(
        &repo,
        "assets/logo.png",
        [0x89, b'P', b'N', b'G', 0, 1, 2, 3, 0, 0xff],
    );
    let mut big = String::new();
    for n in 1..=25_000 {
        writeln!(big, "row {n}").unwrap();
    }
    write(&repo, "data/big.csv", big);
    repo
}

/// The status of the fixture repository and the diff of each change, exactly
/// as the backend returns them.
fn record_fixture() -> Value {
    let harness = Harness::new();
    let repo = fixture_repo(&harness);
    let repo_id = harness.add(&repo);
    let status = harness.ok("get_status", json!({ "repoId": repo_id }));
    let mut diffs = serde_json::Map::new();
    for area in ["staged", "unstaged"] {
        for change in status[area].as_array().unwrap() {
            let diff = harness.ok(
                "get_diff",
                json!({ "repoId": repo_id, "request": {
                    "path": change["path"], "origPath": change["origPath"],
                    "area": area, "kind": change["kind"], "full": false
                }}),
            );
            diffs.insert(format!("{area}:{}", change["path"].as_str().unwrap()), diff);
        }
    }
    json!({ "status": status, "diffs": diffs })
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/e2e/fixtures/changes.json")
}

#[test]
fn fixture_covers_every_kind_of_change() {
    let fixture = record_fixture();
    let diffs = &fixture["diffs"];
    assert!(
        diffs["unstaged:data/big.csv"]["tooLarge"]
            .as_bool()
            .unwrap()
    );
    assert!(
        diffs["unstaged:assets/logo.png"]["binary"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        diffs["unstaged:src/handlers/login.rs"]["hunks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fixture["status"]["staged"][1]["kind"], "renamed");
}

/// Writes `ui/e2e/fixtures/changes.json`, which the E2E fake backend serves.
/// Run via `pnpm fixtures`; CI fails if the committed file is stale.
#[test]
#[ignore = "writes into the source tree; run with `pnpm fixtures`"]
fn write_fixtures() {
    let target = fixture_path();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let json = serde_json::to_string_pretty(&record_fixture()).unwrap() + "\n";
    std::fs::write(target, json).unwrap();
}

/// Every command the frontend wrappers call is registered in the backend.
#[test]
fn registers_every_command_the_frontend_calls() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/api/commands.ts"),
    )
    .unwrap();
    let commands: Vec<&str> = source
        .split("call<")
        .skip(1)
        .filter_map(|rest| rest.split('"').nth(1))
        .collect();
    assert!(commands.len() >= 20, "found only {commands:?}");

    let harness = Harness::new();
    for command in commands {
        // Without arguments most commands fail validation; an unknown command
        // fails differently.
        if let Err(Value::String(message)) = harness.invoke(command, json!({})) {
            assert!(!message.contains("not found"), "{command}: {message}");
        }
    }
}

fn commit_at(repo: &Path, date: &str, file: &str, contents: &str) {
    write(repo, file, contents);
    git_at(repo, date, &["add", "--", file]);
    git_at(
        repo,
        date,
        &["commit", "--quiet", "-m", &format!("update {file}")],
    );
}

/// A repository with a bare `origin` and branches in every relation to
/// `origin/development` and to their upstreams.
fn branch_repo(harness: &Harness) -> PathBuf {
    let remote = harness.dir.path().join("origin.git");
    std::fs::create_dir_all(&remote).unwrap();
    git(
        &remote,
        &["init", "--quiet", "--bare", "--initial-branch=development"],
    );
    let repo = harness.repo("web");
    let url = remote.to_str().unwrap();
    git(&repo, &["remote", "add", "origin", url]);

    commit_at(&repo, "2026-01-01T10:00:00Z", "app.txt", "v1\n");
    git(&repo, &["push", "--quiet", "-u", "origin", "development"]);

    git(&repo, &["switch", "--quiet", "-c", "fix/header-overflow"]);
    commit_at(&repo, "2026-01-02T10:00:00Z", "header.css", "a\n");
    git(
        &repo,
        &["push", "--quiet", "-u", "origin", "fix/header-overflow"],
    );

    git(&repo, &["switch", "--quiet", "development"]);
    commit_at(&repo, "2026-01-03T10:00:00Z", "app.txt", "v2\n");
    commit_at(&repo, "2026-01-04T10:00:00Z", "app.txt", "v3\n");
    git(&repo, &["push", "--quiet", "origin", "development"]);

    git(
        &repo,
        &[
            "switch",
            "--quiet",
            "-c",
            "chore/old-cleanup",
            "fix/header-overflow",
        ],
    );
    git(
        &repo,
        &["push", "--quiet", "-u", "origin", "chore/old-cleanup"],
    );
    git(
        &repo,
        &["push", "--quiet", "origin", "--delete", "chore/old-cleanup"],
    );

    git(
        &repo,
        &["switch", "--quiet", "-c", "spike/local-only", "development"],
    );
    commit_at(&repo, "2026-01-05T10:00:00Z", "spike.txt", "idea\n");

    git(
        &repo,
        &[
            "switch",
            "--quiet",
            "-c",
            "feature/login-rate-limit",
            "development",
        ],
    );
    commit_at(&repo, "2026-01-06T10:00:00Z", "login.txt", "1\n");
    commit_at(&repo, "2026-01-07T10:00:00Z", "login.txt", "2\n");
    git(
        &repo,
        &[
            "push",
            "--quiet",
            "-u",
            "origin",
            "feature/login-rate-limit",
        ],
    );
    commit_at(&repo, "2026-01-08T10:00:00Z", "login.txt", "3\n");
    git(&repo, &["fetch", "--quiet", "--prune"]);
    repo
}

fn find_branch<'a>(list: &'a Value, name: &str) -> &'a Value {
    list["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == name)
        .unwrap_or_else(|| panic!("no branch {name}"))
}

#[test]
fn lists_switches_and_fetches_branches() {
    let harness = Harness::new();
    let repo = branch_repo(&harness);
    let repo_id = harness.add(&repo);

    let list = harness.ok("get_branches", json!({ "repoId": repo_id }));
    assert_eq!(list["mainline"], "origin/development");
    assert_eq!(list["mainlineExists"], true);
    let feature = find_branch(&list, "feature/login-rate-limit");
    assert_eq!(feature["current"], true);
    assert_eq!(feature["mainline"], json!({ "ahead": 3, "behind": 0 }));
    assert_eq!(
        feature["upstream"],
        json!({ "kind": "tracking", "name": "origin/feature/login-rate-limit", "counts": { "ahead": 1, "behind": 0 } })
    );
    assert_eq!(
        find_branch(&list, "fix/header-overflow")["mainline"],
        json!({ "ahead": 1, "behind": 2 })
    );
    assert_eq!(
        find_branch(&list, "chore/old-cleanup")["upstream"]["kind"],
        "gone"
    );
    assert_eq!(
        find_branch(&list, "spike/local-only")["upstream"],
        json!({ "kind": "none" })
    );

    let switched = harness.ok(
        "switch_branch",
        json!({ "repoId": repo_id, "branch": "development" }),
    );
    assert_eq!(find_branch(&switched, "development")["current"], true);

    write(&repo, "app.txt", "local edit\n");
    let refused = harness
        .invoke(
            "switch_branch",
            json!({ "repoId": repo_id, "branch": "fix/header-overflow" }),
        )
        .unwrap_err();
    assert_eq!(refused["kind"], "gitFailed");
    assert_eq!(
        refused["message"],
        "Could not switch to fix/header-overflow."
    );
    assert!(refused["detail"].as_str().unwrap().contains("app.txt"));

    let fetched = harness.ok("fetch_repo", json!({ "repoId": repo_id }));
    assert!(fetched["lastFetched"].is_number());

    let custom = harness.ok(
        "set_mainline",
        json!({ "repoId": repo_id, "mainline": "origin/fix/header-overflow" }),
    );
    assert_eq!(custom["repos"][0]["mainline"], "origin/fix/header-overflow");
    let list = harness.ok("get_branches", json!({ "repoId": repo_id }));
    assert_eq!(
        find_branch(&list, "development")["mainline"],
        json!({ "ahead": 2, "behind": 1 })
    );
    let reset = harness.ok(
        "set_mainline",
        json!({ "repoId": repo_id, "mainline": null }),
    );
    assert_eq!(reset["repos"][0]["mainline"], "origin/development");

    let settings = harness.ok("set_fetch_interval", json!({ "minutes": 0 }));
    assert_eq!(settings["settings"], json!({ "fetchIntervalMinutes": 0 }));
}

/// The branch list of [`branch_repo`] and the commits of each branch;
/// `lastFetched` is cleared because it depends on when the test ran.
fn record_branch_fixture() -> Value {
    let harness = Harness::new();
    let repo = branch_repo(&harness);
    let repo_id = harness.add(&repo);
    let mut list = harness.ok("get_branches", json!({ "repoId": repo_id }));
    list["lastFetched"] = Value::Null;
    let mut commits = serde_json::Map::new();
    for branch in list["branches"].as_array().unwrap() {
        let name = branch["name"].as_str().unwrap();
        commits.insert(
            name.to_owned(),
            harness.ok(
                "get_branch_commits",
                json!({ "repoId": repo_id, "branch": name }),
            ),
        );
    }
    json!({ "list": list, "commits": commits })
}

#[test]
#[ignore = "writes into the source tree; run with `pnpm fixtures`"]
fn write_branch_fixtures() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/e2e/fixtures/branches.json");
    let json = serde_json::to_string_pretty(&record_branch_fixture()).unwrap() + "\n";
    std::fs::write(target, json).unwrap();
}
