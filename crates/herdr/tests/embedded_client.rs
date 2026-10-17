//! The real herdr client inside our PTY, against an isolated herdr server.
//! Needs herdr installed; run with `cargo test -p herdr --test
//! embedded_client -- --ignored`. Never touches the user's own session.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use herdr::{EmbeddedClient, Endpoint};
use pty::{Pty, PtySink, Size};

const SIZE: Size = Size {
    cols: 110,
    rows: 36,
};

/// An isolated herdr server: its own config directory in a short temp dir
/// (Unix socket paths are limited to ~100 bytes).
struct Server {
    dir: tempfile::TempDir,
    binary: PathBuf,
    process: Option<std::process::Child>,
}

impl Server {
    fn start() -> Self {
        let binary = herdr::find_binary(&std::env::var_os("PATH").unwrap_or_default())
            .expect("herdr is not installed");
        let base = if cfg!(unix) {
            PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        };
        let dir = tempfile::Builder::new()
            .prefix("bbh")
            .tempdir_in(base)
            .unwrap();
        let mut server = Self {
            dir,
            binary,
            process: None,
        };
        server.process = Some(
            server
                .herdr()
                .arg("server")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !server.socket().exists() {
            assert!(Instant::now() < deadline, "herdr server did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        server
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("xdg").join("herdr").join("herdr.sock")
    }

    /// The herdr CLI pointed at this server only.
    fn herdr(&self) -> Command {
        let mut command = Command::new(&self.binary);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("HERDR_") {
                command.env_remove(key);
            }
        }
        command
            .env("XDG_CONFIG_HOME", self.dir.path().join("xdg"))
            .env("HERDR_DISABLE_SOUND", "1");
        command
    }

    fn run(&self, args: &[&str]) -> serde_json::Value {
        let output = self.herdr().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "herdr {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or(serde_json::Value::Null)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.herdr().args(["server", "stop"]).output();
        if let Some(mut process) = self.process.take() {
            let _ = process.wait();
        }
    }
}

/// Feeds the client's output into a terminal emulator so tests can read
/// the screen.
struct Screen {
    parser: Mutex<vt100::Parser>,
    exited: Mutex<bool>,
}

impl PtySink for Screen {
    fn output(&self, bytes: Vec<u8>) {
        self.parser.lock().unwrap().process(&bytes);
    }

    fn exited(&self, _code: Option<u32>) {
        *self.exited.lock().unwrap() = true;
    }
}

impl Screen {
    fn text(&self) -> String {
        self.parser.lock().unwrap().screen().contents()
    }

    fn wait_for(&self, what: &str, done: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = self.text();
            if done(&text) {
                return text;
            }
            assert!(Instant::now() < deadline, "{what}; screen:\n{text}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn attach(server: &Server, config: &str) -> (Pty, Arc<Screen>) {
    let path = server
        .dir
        .path()
        .join(format!("client-{}.toml", config.len()));
    std::fs::write(&path, config).unwrap();
    let client = EmbeddedClient {
        binary: server.binary.clone(),
        endpoint: Endpoint::new(server.socket()),
        config: path,
    };
    let screen = Arc::new(Screen {
        parser: Mutex::new(vt100::Parser::new(SIZE.rows, SIZE.cols, 0)),
        exited: Mutex::new(false),
    });
    let mut command = client.command(None);
    command = command.env("XDG_CONFIG_HOME", server.dir.path().join("xdg"));
    let pty = Pty::spawn(&command, SIZE, screen.clone()).unwrap();
    (pty, screen)
}

struct NoSink;

impl herdr::HerdrSink for NoSink {
    fn session_changed(&self, _session: Option<Arc<herdr::Session>>) {}
}

fn workspace(server: &Server, label: &str, mark: &str) -> (PathBuf, String) {
    let dir = server.dir.path().join(label);
    std::fs::create_dir_all(&dir).unwrap();
    let created = server.run(&[
        "workspace",
        "create",
        "--cwd",
        dir.to_str().unwrap(),
        "--label",
        label,
        "--no-focus",
    ]);
    let pane = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_owned();
    server.run(&["pane", "run", &pane, &format!("echo {mark}")]);
    (dir.canonicalize().unwrap(), pane)
}

/// A review pasted into a pane arrives as one bracketed paste, byte for
/// byte, so the agent CLI in it neither submits it nor splits it up.
#[cfg(unix)]
#[test]
#[ignore = "needs herdr installed; starts an isolated herdr server"]
fn pastes_into_a_pane_without_submitting() {
    let server = Server::start();
    let (dir, pane) = workspace(&server, "paste", "PASTE-MARK");
    let received = dir.join("received");
    // A raw terminal hands every byte to `cat` unchanged.
    server.run(&[
        "pane",
        "run",
        &pane,
        &format!("stty raw -icrnl -echo; cat > '{}'", received.display()),
    ]);
    std::thread::sleep(Duration::from_millis(500));

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let link = herdr::HerdrLink::new(Some(Endpoint::new(server.socket())), Arc::new(NoSink));
    runtime
        .block_on(link.paste(&pane, "src/a.ts:3\n```ts\nlet a = 1;\n```\nRename this."))
        .unwrap();

    let expected = "\x1b[200~src/a.ts:3\n```ts\nlet a = 1;\n```\nRename this.\x1b[201~";
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let bytes = std::fs::read(&received).unwrap_or_default();
        if bytes == expected.as_bytes() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the pane received {:?}",
            String::from_utf8_lossy(&bytes)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Gates 2 and 5 of the embedded terminal (see ADR 0013): the real client
/// attaches inside our PTY, hides herdr's sidebar, follows `workspace.focus`
/// and detaches on prefix+q; the agents list covers what herdr lists.
#[test]
#[ignore = "needs herdr installed; starts an isolated herdr server"]
fn embedded_client_works_inside_our_pty() {
    let server = Server::start();
    let (alpha, alpha_pane) = workspace(&server, "alpha", "ALPHA-MARK");
    let (beta, _) = workspace(&server, "beta", "BETA-MARK");
    server.run(&[
        "pane",
        "report-agent",
        &alpha_pane,
        "--source",
        "branchboard-test",
        "--agent",
        "claude",
        "--state",
        "blocked",
    ]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let link = herdr::HerdrLink::new(Some(Endpoint::new(server.socket())), Arc::new(NoSink));
    let repos = herdr::RepoMatcher::new([("alpha", alpha), ("beta", beta)]);
    runtime
        .block_on(link.open(&repos, &"alpha", "alpha"))
        .unwrap();

    // The embedded client shows the focused workspace without a sidebar.
    let (pty, embedded) = attach(&server, &herdr::embedded_config(None));
    let text = embedded.wait_for("embedded client shows alpha", |text| {
        text.contains("ALPHA-MARK")
    });
    assert!(
        !text.contains("beta"),
        "herdr's sidebar is visible:\n{text}"
    );

    // A client with herdr's normal config lists every workspace in its sidebar.
    let (_plain_pty, plain) = attach(&server, "onboarding = false\n");
    plain.wait_for("plain client shows its sidebar", |text| {
        text.contains("alpha") && text.contains("beta")
    });

    // The agents list knows every agent herdr knows.
    runtime.block_on(link.connect());
    let session = link.session().expect("connected");
    let listed = session.agent_list(&repos);
    let herdr_agents = server.run(&["agent", "list"])["result"]["agents"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(listed.len(), herdr_agents);
    assert_eq!(listed[0].0, Some("alpha"));
    assert_eq!(listed[0].1.state, herdr::AgentState::Blocked);

    // Selecting another repository switches the embedded client.
    runtime
        .block_on(link.open(&repos, &"beta", "beta"))
        .unwrap();
    embedded.wait_for("embedded client follows workspace.focus", |text| {
        text.contains("BETA-MARK") && !text.contains("ALPHA-MARK")
    });

    // Switching workspaces inside the client (herdr's navigator, prefix+g) is
    // visible to Branchboard, which then selects the matching repository.
    pty.write(b"\x02g").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    pty.write(b"alpha").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    pty.write(b"\r").unwrap();
    embedded.wait_for("embedded client switched to alpha", |text| {
        text.contains("ALPHA-MARK") && !text.contains("BETA-MARK")
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let session = runtime.block_on(async {
            link.connect().await;
            link.session()
        });
        if session
            .as_ref()
            .and_then(|session| session.focused_repo(&repos))
            == Some("alpha")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "herdr did not report alpha as focused"
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    // prefix+q detaches: the client ends, the server and its panes stay.
    pty.write(b"\x02q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !*embedded.exited.lock().unwrap() {
        assert!(Instant::now() < deadline, "client did not detach");
        std::thread::sleep(Duration::from_millis(50));
    }
    let panes = server.run(&["pane", "list"])["result"]["panes"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(panes, 2);
}
