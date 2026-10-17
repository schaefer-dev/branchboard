//! Contract tests: the real command handlers, invoked through Tauri's IPC
//! layer exactly as the frontend calls them (command names and camelCase
//! arguments from `ui/src/lib/api/commands.ts`), against real repositories.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use branchboard_lib::{Alerts, AppOptions, Notice, Sound, build};
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
        Self::with_options(AppOptions::default())
    }

    #[cfg(unix)]
    fn with_editor(editor: Option<PathBuf>) -> Self {
        Self::with_options(AppOptions {
            editor,
            ..AppOptions::default()
        })
    }

    /// The workspace file always lives in a temp dir, and `herdr_socket`
    /// defaults to a socket nobody listens on, so tests never reach the
    /// developer's own repository list or herdr session.
    fn with_options(options: AppOptions) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let options = AppOptions {
            workspace_file: Some(dir.path().join("workspace.json")),
            herdr_socket: Some(
                options
                    .herdr_socket
                    .unwrap_or_else(|| dir.path().join("herdr.sock")),
            ),
            ..options
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
        // Persist these settings so commands invoked through IPC use them too.
        git(&path, &["config", "--local", "core.autocrlf", "false"]);
        git(&path, &["config", "--local", "core.eol", "lf"]);
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

#[test]
fn commits_and_undoes_commits() {
    let harness = Harness::new();
    let repo = harness.repo("api");
    // Commits made through IPC use the repository's own settings.
    git(
        &repo,
        &["config", "--local", "user.name", "Branchboard Tests"],
    );
    git(
        &repo,
        &[
            "config",
            "--local",
            "user.email",
            "tests@branchboard.invalid",
        ],
    );
    git(&repo, &["config", "--local", "commit.gpgsign", "false"]);
    let repo_id = harness.add(&repo);
    assert_eq!(
        harness.ok("get_head_commit", json!({ "repoId": repo_id })),
        Value::Null
    );

    let empty = harness
        .invoke(
            "commit",
            json!({ "repoId": repo_id, "message": "  ", "amend": false }),
        )
        .unwrap_err();
    assert_eq!(empty["kind"], "invalidInput");

    write(&repo, "a.txt", "a\n");
    harness.ok(
        "stage_paths",
        json!({ "repoId": repo_id, "paths": ["a.txt"] }),
    );
    let first = harness.ok(
        "commit",
        json!({ "repoId": repo_id, "message": "Add a", "amend": false }),
    );
    assert_eq!(first["status"]["staged"], json!([]));
    assert_eq!(first["head"]["subject"], "Add a");
    assert_eq!(first["head"]["hasParent"], false);
    assert_eq!(first["status"]["headOid"], first["head"]["oid"]);

    // Nothing to undo before the first commit.
    let refused = harness
        .invoke("undo_last_commit", json!({ "repoId": repo_id }))
        .unwrap_err();
    assert_eq!(refused["kind"], "invalidInput");

    write(&repo, "b.txt", "b\n");
    harness.ok(
        "stage_paths",
        json!({ "repoId": repo_id, "paths": ["b.txt"] }),
    );
    let second = harness.ok(
        "commit",
        json!({ "repoId": repo_id, "message": "Add b\n\nDetails.", "amend": false }),
    );
    assert_eq!(second["head"]["message"], "Add b\n\nDetails.");
    let head = harness.ok("get_head_commit", json!({ "repoId": repo_id }));
    assert_eq!(head["oid"], second["head"]["oid"]);
    assert_eq!(head["pushed"], false);

    let amended = harness.ok(
        "commit",
        json!({ "repoId": repo_id, "message": "Add b and more", "amend": true }),
    );
    assert_eq!(amended["head"]["subject"], "Add b and more");

    let undone = harness.ok("undo_last_commit", json!({ "repoId": repo_id }));
    assert_eq!(undone["message"], "Add b and more");
    assert_eq!(undone["head"]["oid"], first["head"]["oid"]);
    assert_eq!(
        paths(&undone["status"]["staged"]),
        vec![("b.txt".into(), "added".into())]
    );
}

#[cfg(unix)]
#[test]
fn reports_failing_commit_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let harness = Harness::new();
    let repo = harness.repo("api");
    git(
        &repo,
        &["config", "--local", "user.name", "Branchboard Tests"],
    );
    git(
        &repo,
        &[
            "config",
            "--local",
            "user.email",
            "tests@branchboard.invalid",
        ],
    );
    let hook = repo.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho 'lint: 2 problems'\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    write(&repo, "a.txt", "a\n");
    let repo_id = harness.add(&repo);
    harness.ok(
        "stage_paths",
        json!({ "repoId": repo_id, "paths": ["a.txt"] }),
    );

    let error = harness
        .invoke(
            "commit",
            json!({ "repoId": repo_id, "message": "Add a", "amend": false }),
        )
        .unwrap_err();
    assert_eq!(error["kind"], "gitFailed");
    assert_eq!(error["message"], "Commit failed.");
    assert!(
        error["detail"]
            .as_str()
            .unwrap()
            .contains("lint: 2 problems")
    );
}

/// A repository with one change of every kind: staged and unstaged edits,
/// a rename, a deletion, untracked text, binary and very large files.
fn fixture_repo(harness: &Harness) -> PathBuf {
    let repo = harness.repo("api");
    let mut login = [
        "import { loadUsers, verify } from \"./users\";",
        "",
        "/** Handles login requests. */",
        "export function login(user: string, password: string): Session {",
        "  const users = loadUsers();",
        "  const hash = users.get(user);",
        "  if (!hash) throw new UnknownUser();",
        "  // checked below",
        "  verify(hash, password);",
        "  return new Session(user);",
        "}",
    ]
    .join("\n");
    login.push('\n');
    for n in 1..=40 {
        writeln!(login, "// line {n}").unwrap();
    }
    write(&repo, "src/handlers/login.ts", &login);
    write(&repo, "config.toml", "[server]\nport = 8080\n");
    write(&repo, "legacy.txt", "old notes\n");
    write(&repo, "README.md", "# api\n");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);

    let edited = login
        .replace(
            "  const users = loadUsers();\n",
            "  const users = loadUsers({ cache: true });\n  log.debug(\"login attempt\", { user });\n",
        )
        .replace("new UnknownUser()", "new UnknownUser(user)")
        .replace("// line 30\n", "// line 30 (changed)\n");
    write(&repo, "src/handlers/login.ts", edited);
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
        diffs["unstaged:src/handlers/login.ts"]["hunks"]
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

const RATE_LIMIT_V1: &str = "export const LIMIT = 10;

export function allow(hits: number): boolean {
  return hits < LIMIT;
}
";

const RATE_LIMIT_V2: &str = "/** Requests per user and minute. */
export const LIMIT = 60;

export function allow(hits: number): boolean {
  return hits < LIMIT;
}

export function headers(hits: number): Record<string, string> {
  return { \"RateLimit-Remaining\": String(Math.max(0, LIMIT - hits)) };
}
";

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
    // Reviewing the branch shows more than one file and a real diff.
    write(&repo, "src/rate-limit.ts", RATE_LIMIT_V1);
    git(&repo, &["add", "src/rate-limit.ts"]);
    commit_at(&repo, "2026-01-06T10:00:00Z", "login.txt", "1\n");
    write(&repo, "src/rate-limit.ts", RATE_LIMIT_V2);
    git(&repo, &["add", "src/rate-limit.ts"]);
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
    // No choice stored: the branch list resolves the automatic mainline.
    assert_eq!(reset["repos"][0]["mainline"], Value::Null);
    let list = harness.ok("get_branches", json!({ "repoId": repo_id }));
    assert_eq!(list["mainline"], "origin/development");

    let settings = harness.ok("set_fetch_interval", json!({ "minutes": 0 }));
    assert_eq!(settings["settings"]["fetchIntervalMinutes"], 0);
}

#[test]
fn reviews_branches_and_commits() {
    let harness = Harness::new();
    let repo = branch_repo(&harness);
    let repo_id = harness.add(&repo);

    let branch = harness.ok(
        "get_review_files",
        json!({ "repoId": repo_id, "target": { "kind": "branch", "name": "fix/header-overflow" } }),
    );
    // Only what the branch adds, not what the mainline gained since.
    assert_eq!(
        paths(&branch["files"]),
        vec![("header.css".into(), "added".into())]
    );
    assert_eq!(branch["files"][0]["additions"], 1);

    let commits = harness.ok(
        "get_branch_commits",
        json!({ "repoId": repo_id, "branch": "feature/login-rate-limit" }),
    );
    let oid = &commits["ahead"]["commits"][0]["oid"];
    let commit = harness.ok(
        "get_review_files",
        json!({ "repoId": repo_id, "target": { "kind": "commit", "oid": oid } }),
    );
    assert_eq!(commit["range"]["to"], *oid);
    assert_eq!(
        paths(&commit["files"]),
        vec![("login.txt".into(), "modified".into())]
    );

    let diff = harness.ok(
        "get_review_diff",
        json!({ "repoId": repo_id, "request": {
            "range": commit["range"], "path": "login.txt", "origPath": null, "full": false
        }}),
    );
    assert_eq!(diff["hunks"][0]["lines"][0]["text"], "2");
    assert_eq!(diff["hunks"][0]["lines"][1]["text"], "3");

    let refused = harness
        .invoke(
            "get_review_diff",
            json!({ "repoId": repo_id, "request": {
                "range": { "from": "--output=x", "to": oid }, "path": "login.txt",
                "origPath": null, "full": false
            }}),
        )
        .unwrap_err();
    assert_eq!(refused["kind"], "invalidInput");
}

/// The review of `target`, and the diff of each of its files keyed by
/// `<from>..<to>:<path>`.
fn record_review(
    harness: &Harness,
    repo_id: &str,
    target: &Value,
    diffs: &mut serde_json::Map<String, Value>,
) -> Value {
    let review = harness.ok(
        "get_review_files",
        json!({ "repoId": repo_id, "target": target }),
    );
    let range = &review["range"];
    for file in review["files"].as_array().unwrap() {
        let key = format!(
            "{}..{}:{}",
            range["from"].as_str().unwrap(),
            range["to"].as_str().unwrap(),
            file["path"].as_str().unwrap()
        );
        let diff = harness.ok(
            "get_review_diff",
            json!({ "repoId": repo_id, "request": {
                "range": range, "path": file["path"], "origPath": file["origPath"], "full": false
            }}),
        );
        diffs.insert(key, diff);
    }
    review
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
    let mut reviews = serde_json::Map::new();
    let mut review_diffs = serde_json::Map::new();
    for branch in list["branches"].as_array().unwrap() {
        let name = branch["name"].as_str().unwrap();
        let branch_commits = harness.ok(
            "get_branch_commits",
            json!({ "repoId": repo_id, "branch": name }),
        );
        let target = json!({ "kind": "branch", "name": name });
        let review = record_review(&harness, &repo_id, &target, &mut review_diffs);
        reviews.insert(format!("branch:{name}"), review);
        for side in ["ahead", "behind"] {
            let Some(listed) = branch_commits[side]["commits"].as_array() else {
                continue;
            };
            for commit in listed {
                let oid = commit["oid"].as_str().unwrap();
                let target = json!({ "kind": "commit", "oid": oid });
                let review = record_review(&harness, &repo_id, &target, &mut review_diffs);
                reviews.insert(format!("commit:{oid}"), review);
            }
        }
        commits.insert(name.to_owned(), branch_commits);
    }
    json!({ "list": list, "commits": commits, "reviews": reviews, "reviewDiffs": review_diffs })
}

#[test]
#[ignore = "writes into the source tree; run with `pnpm fixtures`"]
fn write_branch_fixtures() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/e2e/fixtures/branches.json");
    let json = serde_json::to_string_pretty(&record_branch_fixture()).unwrap() + "\n";
    std::fs::write(target, json).unwrap();
}

#[cfg(unix)]
#[test]
fn opens_files_in_the_editor() {
    use std::os::unix::fs::PermissionsExt;

    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("args.txt");
    let editor = scripts.path().join("editor.sh");
    std::fs::write(
        &editor,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    let harness = Harness::with_editor(Some(editor));
    let repo = harness.repo("api");
    let repo_id = harness.add(&repo);
    let result = harness.ok(
        "open_in_editor",
        json!({ "repoId": repo_id, "path": "src/main.rs", "line": 12 }),
    );
    assert_eq!(result, Value::Null);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !log.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "editor was not started"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let args = std::fs::read_to_string(&log).unwrap();
    let expected = format!(
        "{}\n--goto\n{}:12\n",
        repo.display(),
        repo.join("src").join("main.rs").display()
    );
    assert_eq!(args, expected);
}

/// A connection to the fake herdr; writes from the connection's thread and
/// from pushed events must not interleave.
struct FakeConnection {
    stream: interprocess::local_socket::Stream,
    writing: std::sync::Mutex<()>,
}

impl FakeConnection {
    fn send(&self, line: &Value) -> std::io::Result<()> {
        use std::io::Write;
        let _writing = self.writing.lock().unwrap();
        (&self.stream).write_all(format!("{line}\n").as_bytes())
    }
}

/// A herdr server answering from a session snapshot the test can change;
/// records the requests it gets.
struct FakeHerdr {
    socket: PathBuf,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    snapshot: std::sync::Arc<std::sync::Mutex<Value>>,
    subscribers: std::sync::Arc<std::sync::Mutex<Vec<std::sync::Arc<FakeConnection>>>>,
    _dir: TempDir,
}

impl FakeHerdr {
    fn start(snapshot: Value) -> Self {
        use interprocess::local_socket::ListenerOptions;
        use interprocess::local_socket::prelude::*;
        use std::io::{BufRead, BufReader};
        use std::sync::{Arc, Mutex};

        // Unix socket paths are limited to ~100 bytes; macOS temp dirs are long.
        let dir = if cfg!(unix) {
            tempfile::Builder::new().tempdir_in("/tmp").unwrap()
        } else {
            tempfile::tempdir().unwrap()
        };
        let socket = dir.path().join("herdr.sock");
        #[cfg(unix)]
        let name = {
            use interprocess::local_socket::{GenericFilePath, ToFsName as _};
            socket.clone().to_fs_name::<GenericFilePath>().unwrap()
        };
        #[cfg(windows)]
        let name = {
            use interprocess::local_socket::{GenericNamespaced, ToNsName as _};
            socket
                .to_string_lossy()
                .into_owned()
                .to_ns_name::<GenericNamespaced>()
                .unwrap()
        };
        let listener = ListenerOptions::new().name(name).create_sync().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let snapshot = Arc::new(Mutex::new(snapshot));
        let subscribers: Arc<Mutex<Vec<Arc<FakeConnection>>>> = Arc::default();
        let (log, current, subscribed) = (
            Arc::clone(&requests),
            Arc::clone(&snapshot),
            Arc::clone(&subscribers),
        );
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let connection = Arc::new(FakeConnection {
                    stream,
                    writing: Mutex::new(()),
                });
                let (log, current, subscribed) = (
                    Arc::clone(&log),
                    Arc::clone(&current),
                    Arc::clone(&subscribed),
                );
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(&connection.stream);
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap_or(0) > 0 {
                        let request: Value = serde_json::from_str(&line).unwrap();
                        line.clear();
                        let result = match request["method"].as_str().unwrap() {
                            "session.snapshot" => {
                                let snapshot = current.lock().unwrap().clone();
                                json!({ "type": "session_snapshot", "snapshot": snapshot })
                            }
                            "events.subscribe" => {
                                subscribed.lock().unwrap().push(Arc::clone(&connection));
                                json!({ "type": "subscription_started" })
                            }
                            "workspace.create" => json!({ "workspace": { "workspace_id": "w9" } }),
                            _ => json!({ "type": "ok" }),
                        };
                        let response = json!({ "id": request["id"], "result": result });
                        log.lock().unwrap().push(request);
                        if connection.send(&response).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        Self {
            socket,
            requests,
            snapshot,
            subscribers,
            _dir: dir,
        }
    }

    /// Changes the session and tells the subscribers, like herdr does when
    /// an agent's state changes.
    fn change(&self, change: impl FnOnce(&mut Value)) {
        change(&mut self.snapshot.lock().unwrap());
        let event = json!({ "event": "pane.agent_status_changed", "data": {} });
        self.subscribers
            .lock()
            .unwrap()
            .retain(|connection| connection.send(&event).is_ok());
    }

    fn requests(&self, method: &str) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request["method"] == method)
            .map(|request| request["params"].clone())
            .collect()
    }
}

#[test]
fn reports_agents_unavailable_without_herdr() {
    let harness = Harness::new();
    let repo_id = harness.add(&harness.repo("api"));
    assert_eq!(
        harness.ok("get_agents", json!({})),
        json!({ "available": false, "repos": [], "agents": [], "focusedRepoId": null })
    );
    let error = harness
        .invoke("open_in_herdr", json!({ "repoId": repo_id }))
        .unwrap_err();
    assert_eq!(error["kind"], "herdrUnavailable");
}

/// Records alerts instead of showing notifications and playing sounds.
#[derive(Debug, Default)]
struct RecordedAlerts {
    notices: std::sync::Mutex<Vec<Notice>>,
    sounds: std::sync::Mutex<Vec<Sound>>,
}

impl Alerts for RecordedAlerts {
    fn notify(&self, notice: Notice) {
        self.notices.lock().unwrap().push(notice);
    }

    fn play(&self, sound: Sound) {
        self.sounds.lock().unwrap().push(sound);
    }
}

impl RecordedAlerts {
    fn sounds(&self) -> Vec<Sound> {
        self.sounds.lock().unwrap().clone()
    }

    /// Waits for `count` sounds, then a moment longer for any unexpected one.
    fn wait_for_sounds(&self, count: usize) -> Vec<Sound> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.sounds().len() < count {
            assert!(
                std::time::Instant::now() < deadline,
                "only got {:?}",
                self.sounds()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
        self.sounds()
    }
}

/// Two repositories and a fake herdr with agents in one of them and in a
/// space that is no repository.
struct HerdrSetup {
    fake: FakeHerdr,
    harness: Harness,
    alerts: std::sync::Arc<RecordedAlerts>,
    api_id: String,
    docs_id: String,
    _repos: TempDir,
}

fn herdr_setup() -> HerdrSetup {
    // The repositories must exist before the snapshot can name their paths.
    let repos = tempfile::tempdir().unwrap();
    let api = repos.path().join("api");
    let docs = repos.path().join("docs");
    for path in [&api, &docs] {
        std::fs::create_dir_all(path).unwrap();
        git(path, &["init", "--quiet", "--initial-branch=development"]);
    }
    let api = api.canonicalize().unwrap();
    let api_src = api.join("src");
    let fake = FakeHerdr::start(json!({
        "version": "0.9.3",
        "protocol": 22,
        "workspaces": [
            { "workspace_id": "w1", "number": 1, "label": "api" },
            { "workspace_id": "w2", "number": 2, "label": "notes" },
        ],
        "tabs": [{ "tab_id": "w1:t1", "workspace_id": "w1", "label": "agents" }],
        "panes": [
            { "pane_id": "w1:p1", "workspace_id": "w1", "cwd": api },
            { "pane_id": "w1:p2", "workspace_id": "w1", "cwd": api_src },
        ],
        "agents": [
            {
                "pane_id": "w1:p2", "workspace_id": "w1", "agent": "claude",
                "agent_status": "blocked", "cwd": api_src,
                "terminal_title_stripped": "Fix the login form",
            },
            { "pane_id": "w1:p1", "workspace_id": "w1", "tab_id": "w1:t1", "agent": "codex", "agent_status": "working", "cwd": api },
            { "pane_id": "w2:p1", "workspace_id": "w2", "agent": "claude", "agent_status": "done", "cwd": "/notes" },
        ],
    }));

    let alerts = std::sync::Arc::new(RecordedAlerts::default());
    let harness = Harness::with_options(AppOptions {
        herdr_socket: Some(fake.socket.clone()),
        alerts: Some(alerts.clone()),
        alert_delay: Some(std::time::Duration::ZERO),
        ..AppOptions::default()
    });
    let api_id = harness.add(&api);
    let docs_id = harness.add(&docs);

    HerdrSetup {
        fake,
        harness,
        alerts,
        api_id,
        docs_id,
        _repos: repos,
    }
}

/// Sets the state of the agent in `pane_id` in a fake session snapshot.
fn set_agent_state(snapshot: &mut Value, pane_id: &str, state: &str) {
    let agent = snapshot["agents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|agent| agent["pane_id"] == pane_id)
        .unwrap();
    agent["agent_status"] = json!(state);
}

#[test]
fn pastes_into_an_agent_without_submitting() {
    let HerdrSetup { fake, harness, .. } = herdr_setup();
    harness.ok(
        "paste_to_agent",
        json!({ "paneId": "w1:p1", "text": "src/a.ts:3\nRename this." }),
    );
    assert_eq!(
        fake.requests("pane.send_text"),
        [json!({ "pane_id": "w1:p1", "text": "\u{1b}[200~src/a.ts:3\nRename this.\u{1b}[201~" })]
    );
}

#[test]
fn refuses_to_paste_into_blocked_or_missing_agents() {
    let HerdrSetup { fake, harness, .. } = herdr_setup();
    for pane in ["w1:p2", "missing"] {
        assert!(
            harness
                .invoke(
                    "paste_to_agent",
                    json!({ "paneId": pane, "text": "Review" })
                )
                .is_err()
        );
    }
    assert_eq!(fake.requests("pane.send_text"), Vec::<Value>::new());
}

#[test]
fn alerts_when_agents_come_to_need_the_user() {
    let HerdrSetup {
        fake,
        harness,
        alerts,
        ..
    } = herdr_setup();
    // The first session only sets the baseline: its blocked and done agents
    // were there before.
    harness.ok("get_agents", json!({}));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(alerts.sounds(), []);

    // The working Codex finishes; the window has focus, so no notification.
    fake.change(|snapshot| set_agent_state(snapshot, "w1:p1", "done"));
    assert_eq!(alerts.wait_for_sounds(1), [Sound::Done]);
    assert_eq!(alerts.notices.lock().unwrap().len(), 0);

    // An agent the user watches in the terminal stays silent by default…
    harness.ok("set_terminal_visible", json!({ "visible": true }));
    fake.change(|snapshot| {
        snapshot["focused_workspace_id"] = json!("w1");
        set_agent_state(snapshot, "w1:p1", "blocked");
    });
    std::thread::sleep(std::time::Duration::from_millis(400));
    // …one in another workspace does not.
    fake.change(|snapshot| set_agent_state(snapshot, "w2:p1", "blocked"));
    assert_eq!(alerts.wait_for_sounds(2), [Sound::Done, Sound::NeedsInput]);

    // Muted, nothing plays.
    let workspace = harness.ok("set_sound_muted", json!({ "muted": true }));
    assert_eq!(workspace["settings"]["soundMuted"], true);
    fake.change(|snapshot| set_agent_state(snapshot, "w2:p1", "done"));
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert_eq!(alerts.sounds().len(), 2);
}

#[test]
fn remembers_the_sound_settings() {
    let harness = Harness::new();
    let workspace = harness.ok("get_workspace", json!({}));
    assert_eq!(workspace["settings"]["soundMuted"], false);
    assert_eq!(workspace["settings"]["muteWhenWatching"], true);
    let workspace = harness.ok("set_sound_muted", json!({ "muted": true }));
    assert_eq!(workspace["settings"]["soundMuted"], true);
    let workspace = harness.ok("set_mute_when_watching", json!({ "mute": false }));
    assert_eq!(workspace["settings"]["muteWhenWatching"], false);
}

#[test]
fn reports_agents_from_herdr() {
    let HerdrSetup {
        fake,
        harness,
        api_id,
        ..
    } = herdr_setup();
    let agents = harness.ok("get_agents", json!({}));
    assert_eq!(
        agents,
        json!({
            "available": true,
            "focusedRepoId": null,
            "repos": [{
                "repoId": api_id,
                "state": "blocked",
                "agents": [
                    { "paneId": "w1:p2", "agent": "claude", "state": "blocked", "title": "Fix the login form" },
                    { "paneId": "w1:p1", "agent": "codex", "state": "working", "title": null },
                ],
            }],
            "agents": [
                {
                    "paneId": "w1:p2", "agent": "claude", "state": "blocked",
                    "title": "Fix the login form", "workspace": "api", "tab": null, "repoId": api_id,
                },
                {
                    "paneId": "w2:p1", "agent": "claude", "state": "done",
                    "title": null, "workspace": "notes", "tab": null, "repoId": null,
                },
                {
                    "paneId": "w1:p1", "agent": "codex", "state": "working",
                    "title": null, "workspace": "api", "tab": "agents", "repoId": api_id,
                },
            ],
        })
    );
    // Listing connected and subscribed to herdr's events.
    assert_eq!(fake.requests("events.subscribe").len(), 1);
}

#[test]
fn opens_workspaces_and_agents_in_herdr() {
    let HerdrSetup {
        fake,
        harness,
        api_id,
        docs_id,
        ..
    } = herdr_setup();
    assert_eq!(
        harness.ok("focus_agent", json!({ "paneId": "w2:p1" })),
        Value::Null
    );
    assert_eq!(fake.requests("agent.focus"), [json!({ "target": "w2:p1" })]);

    assert_eq!(
        harness.ok("open_in_herdr", json!({ "repoId": api_id })),
        Value::Null
    );
    assert_eq!(
        fake.requests("workspace.focus"),
        [json!({ "workspace_id": "w1" })]
    );

    // Focusing never creates a workspace.
    assert_eq!(
        harness.ok("focus_repo_in_herdr", json!({ "repoId": docs_id })),
        json!(false)
    );
    assert_eq!(
        harness.ok("focus_repo_in_herdr", json!({ "repoId": api_id })),
        json!(true)
    );
    assert_eq!(fake.requests("workspace.create"), Vec::<Value>::new());

    harness.ok("open_in_herdr", json!({ "repoId": docs_id }));
    // The path as registered (Windows canonical paths differ in form).
    let workspace = harness.ok("get_workspace", json!({}));
    let docs_path = workspace["repos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|repo| repo["id"] == docs_id.as_str())
        .unwrap()["path"]
        .clone();
    assert_eq!(
        fake.requests("workspace.create"),
        [json!({ "cwd": docs_path, "label": "docs", "focus": true })]
    );
}

/// Waits until `path` exists and holds text for which `done` is true.
#[cfg(unix)]
fn wait_for_file(path: &Path, done: impl Fn(&str) -> bool) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if done(&text) {
            return text;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{} never became ready: {text:?}",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn runs_the_embedded_terminal() {
    use std::os::unix::fs::PermissionsExt;

    // Stands in for herdr: records its environment, terminal size and input.
    let scripts = tempfile::tempdir().unwrap();
    let out = scripts.path();
    let program = out.join("herdr");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nenv > '{0}/env.txt'\nwhile read line; do\n  \
             [ \"$line\" = size ] && stty size > '{0}/size.txt'\n  \
             echo \"$line\" >> '{0}/input.txt'\ndone\n",
            out.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

    let harness = Harness::with_options(AppOptions {
        terminal_program: Some(program),
        ..AppOptions::default()
    });
    assert_eq!(
        harness.ok("terminal_info", json!({})),
        json!({ "available": true })
    );
    harness.ok(
        "terminal_open",
        json!({ "cols": 80, "rows": 24, "output": "__CHANNEL__:1" }),
    );

    let env = wait_for_file(&out.join("env.txt"), |text| text.contains("TERM="));
    let config = harness.dir.path().join("herdr-embedded.toml");
    assert!(
        env.contains(&format!("HERDR_CONFIG_PATH={}", config.display())),
        "{env}"
    );
    assert!(env.contains(&format!(
        "HERDR_SOCKET_PATH={}",
        harness.dir.path().join("herdr.sock").display()
    )));
    assert!(env.contains("TERM=xterm-256color"));
    assert!(!env.contains("HERDR_PANE_ID="), "{env}");
    let written = std::fs::read_to_string(&config).unwrap();
    assert!(
        written.contains("sidebar_collapsed_mode = \"hidden\""),
        "{written}"
    );

    harness.ok("terminal_write", json!({ "data": "hello\r" }));
    wait_for_file(&out.join("input.txt"), |text| text.contains("hello"));
    harness.ok("terminal_resize", json!({ "cols": 100, "rows": 30 }));
    harness.ok("terminal_write", json!({ "data": "size\r" }));
    wait_for_file(&out.join("size.txt"), |text| text.trim() == "30 100");

    harness.ok("terminal_close", json!({}));
    let error = harness
        .invoke("terminal_write", json!({ "data": "late\r" }))
        .unwrap_err();
    assert_eq!(error["kind"], "terminalFailed");
}

#[test]
fn remembers_the_terminal_font_size() {
    let harness = Harness::new();
    let workspace = harness.ok("get_workspace", json!({}));
    assert_eq!(workspace["settings"]["terminalFontSize"], 17);
    let workspace = harness.ok("set_terminal_font_size", json!({ "size": 20 }));
    assert_eq!(workspace["settings"]["terminalFontSize"], 20);
    // Out-of-range sizes are kept within bounds.
    let workspace = harness.ok("set_terminal_font_size", json!({ "size": 200 }));
    assert_eq!(workspace["settings"]["terminalFontSize"], 32);
}
