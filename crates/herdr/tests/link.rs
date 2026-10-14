//! The link against a fake herdr server speaking the socket protocol:
//! newline-delimited JSON requests, responses and pushed events.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use herdr::{AgentState, Endpoint, HerdrLink, HerdrSink, Opened, RepoMatcher, Session};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::prelude::*;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

const FIXTURE: &str = include_str!("fixtures/session.json");

/// The fake server's state, shared with the test.
#[derive(Clone, Default)]
struct Fake {
    snapshot: Arc<Mutex<Value>>,
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    subscribers: Arc<Mutex<Vec<mpsc::UnboundedSender<Option<String>>>>>,
}

impl Fake {
    fn start(path: &std::path::Path) -> Self {
        let fake = Self::default();
        *fake.snapshot.lock().unwrap() =
            serde_json::from_str::<Value>(FIXTURE).unwrap()["snapshot"].clone();

        #[cfg(unix)]
        let name = {
            use interprocess::local_socket::{GenericFilePath, ToFsName as _};
            path.to_path_buf().to_fs_name::<GenericFilePath>().unwrap()
        };
        #[cfg(windows)]
        let name = {
            use interprocess::local_socket::{GenericNamespaced, ToNsName as _};
            path.to_string_lossy()
                .into_owned()
                .to_ns_name::<GenericNamespaced>()
                .unwrap()
        };
        let listener = ListenerOptions::new().name(name).create_tokio().unwrap();
        let server = fake.clone();
        tokio::spawn(async move {
            while let Ok(stream) = listener.accept().await {
                tokio::spawn(server.clone().serve(stream));
            }
        });
        fake
    }

    async fn serve(self, stream: interprocess::local_socket::tokio::Stream) {
        let (reader, mut writer) = stream.split();
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let request: Value = serde_json::from_str(&line).unwrap();
            let id = request["id"].clone();
            let method = request["method"].as_str().unwrap().to_owned();
            self.calls
                .lock()
                .unwrap()
                .push((method.clone(), request["params"].clone()));
            let result = match method.as_str() {
                "session.snapshot" => {
                    json!({ "type": "session_snapshot", "snapshot": *self.snapshot.lock().unwrap() })
                }
                "events.subscribe" => {
                    let ack = json!({ "id": id, "result": { "type": "subscription_started" } });
                    send(&mut writer, &ack).await;
                    // From now on this connection only carries pushed lines;
                    // `None` closes it.
                    let (tx, mut rx) = mpsc::unbounded_channel();
                    self.subscribers.lock().unwrap().push(tx);
                    while let Some(Some(line)) = rx.recv().await {
                        writer.write_all(line.as_bytes()).await.unwrap();
                    }
                    return;
                }
                "workspace.focus" | "agent.focus" => json!({ "type": "ok" }),
                "workspace.create" => json!({
                    "type": "workspace_created",
                    "workspace": { "workspace_id": "w9" },
                }),
                _ => {
                    let error = json!({ "id": id, "error": { "code": "invalid_request", "message": method } });
                    send(&mut writer, &error).await;
                    continue;
                }
            };
            send(&mut writer, &json!({ "id": id, "result": result })).await;
        }
    }

    fn push(&self, line: &Value) {
        for subscriber in self.subscribers.lock().unwrap().iter() {
            let _ = subscriber.send(Some(format!("{line}\n")));
        }
    }

    fn close_subscriptions(&self) {
        for subscriber in self.subscribers.lock().unwrap().drain(..) {
            let _ = subscriber.send(None);
        }
    }

    fn set_status(&self, pane_id: &str, status: &str) {
        let mut snapshot = self.snapshot.lock().unwrap();
        for agent in snapshot["agents"].as_array_mut().unwrap() {
            if agent["pane_id"] == pane_id {
                agent["agent_status"] = json!(status);
            }
        }
    }

    fn calls(&self, method: &str) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params.clone())
            .collect()
    }
}

async fn send(writer: &mut interprocess::local_socket::tokio::SendHalf, value: &Value) {
    writer
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
}

struct Sink(mpsc::UnboundedSender<Option<Arc<Session>>>);

impl HerdrSink for Sink {
    fn session_changed(&self, session: Option<Arc<Session>>) {
        let _ = self.0.send(session);
    }
}

struct Setup {
    fake: Fake,
    link: HerdrLink,
    updates: mpsc::UnboundedReceiver<Option<Arc<Session>>>,
    _dir: tempfile::TempDir,
}

fn socket_dir() -> tempfile::TempDir {
    // Unix socket paths are limited to ~100 bytes; macOS temp dirs are long.
    if cfg!(unix) {
        tempfile::Builder::new().tempdir_in("/tmp").unwrap()
    } else {
        tempfile::tempdir().unwrap()
    }
}

fn setup() -> Setup {
    let dir = socket_dir();
    let path = dir.path().join("herdr.sock");
    let fake = Fake::start(&path);
    let (tx, updates) = mpsc::unbounded_channel();
    let link = HerdrLink::new(Some(Endpoint::new(path)), Arc::new(Sink(tx)));
    Setup {
        fake,
        link,
        updates,
        _dir: dir,
    }
}

async fn next(updates: &mut mpsc::UnboundedReceiver<Option<Arc<Session>>>) -> Option<Arc<Session>> {
    tokio::time::timeout(Duration::from_secs(5), updates.recv())
        .await
        .expect("no update from the link")
        .expect("sink closed")
}

fn status(session: &Session, pane_id: &str) -> AgentState {
    session
        .agents
        .iter()
        .find(|agent| agent.pane_id == pane_id)
        .unwrap()
        .agent_status
}

fn repos() -> RepoMatcher<&'static str> {
    RepoMatcher::new([
        ("api", PathBuf::from("/home/dev/api")),
        ("docs", PathBuf::from("/home/dev/docs")),
    ])
}

#[tokio::test]
async fn unavailable_without_a_server() {
    let dir = socket_dir();
    let (tx, mut updates) = mpsc::unbounded_channel();
    let link = HerdrLink::new(
        Some(Endpoint::new(dir.path().join("herdr.sock"))),
        Arc::new(Sink(tx)),
    );
    link.connect().await;
    assert!(link.session().is_none());
    // Nothing changed (unavailable before and after), so nothing was sent.
    assert!(updates.try_recv().is_err());

    let error = link.open(&repos(), &"api", "api").await.unwrap_err();
    assert!(matches!(error, herdr::HerdrError::NotRunning), "{error:?}");
}

#[tokio::test]
async fn subscribes_to_agent_state_of_every_pane() {
    let mut setup = setup();
    setup.link.connect().await;
    let session = next(&mut setup.updates).await.unwrap();
    assert_eq!(session.workspaces.len(), 5);
    assert_eq!(setup.link.session().as_deref(), Some(&*session));

    let subscribe = setup.fake.calls("events.subscribe");
    let subscriptions = subscribe[0]["subscriptions"].as_array().unwrap();
    let watched: Vec<&str> = subscriptions
        .iter()
        .filter(|entry| entry["type"] == "pane.agent_status_changed")
        .map(|entry| entry["pane_id"].as_str().unwrap())
        .collect();
    assert_eq!(watched.len(), session.panes.len());
    assert!(
        subscriptions
            .iter()
            .any(|entry| entry["type"] == "pane.created")
    );

    // Connecting again while connected does nothing.
    setup.link.connect().await;
    assert_eq!(setup.fake.calls("events.subscribe").len(), 1);
}

#[tokio::test]
async fn events_lead_to_a_new_snapshot() {
    let mut setup = setup();
    setup.link.connect().await;
    let session = next(&mut setup.updates).await.unwrap();
    assert_eq!(status(&session, "w1:p2"), AgentState::Blocked);

    setup.fake.set_status("w1:p2", "working");
    setup.fake.push(&json!({
        "event": "pane.agent_status_changed",
        "data": { "pane_id": "w1:p2", "agent_status": "working" },
    }));
    let session = next(&mut setup.updates).await.unwrap();
    assert_eq!(status(&session, "w1:p2"), AgentState::Working);
}

#[tokio::test]
async fn a_new_pane_renews_the_subscription() {
    let mut setup = setup();
    setup.link.connect().await;
    next(&mut setup.updates).await.unwrap();

    setup.fake.snapshot.lock().unwrap()["panes"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "pane_id": "w2:p3", "workspace_id": "w2", "cwd": "/home/dev/web" }));
    setup
        .fake
        .push(&json!({ "event": "pane.created", "data": { "pane_id": "w2:p3" } }));
    let session = next(&mut setup.updates).await.unwrap();
    assert_eq!(session.panes.len(), 11);

    let subscribe = setup.fake.calls("events.subscribe");
    assert_eq!(subscribe.len(), 2);
    assert!(
        subscribe[1]["subscriptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["pane_id"] == "w2:p3")
    );
}

#[tokio::test]
async fn lost_events_resubscribe() {
    let mut setup = setup();
    setup.link.connect().await;
    next(&mut setup.updates).await.unwrap();

    setup.fake.set_status("w1:p3", "done");
    setup
        .fake
        .push(&json!({ "id": "", "error": { "code": "events_lost", "message": "slow" } }));
    let session = next(&mut setup.updates).await.unwrap();
    assert_eq!(status(&session, "w1:p3"), AgentState::Done);
    assert_eq!(setup.fake.calls("events.subscribe").len(), 2);
}

#[tokio::test]
async fn reconnects_after_the_server_went_away() {
    let mut setup = setup();
    setup.link.connect().await;
    next(&mut setup.updates).await.unwrap();

    setup.fake.close_subscriptions();
    assert!(next(&mut setup.updates).await.is_none());
    assert!(setup.link.session().is_none());

    // The next window focus connects again.
    setup.link.connect().await;
    assert!(next(&mut setup.updates).await.is_some());
}

#[tokio::test]
async fn open_focuses_the_repository_workspace() {
    let setup = setup();
    let opened = setup.link.open(&repos(), &"api", "api").await.unwrap();
    assert_eq!(
        opened,
        Opened::Focused {
            workspace_id: "w1".into()
        }
    );
    assert_eq!(
        setup.fake.calls("workspace.focus"),
        [json!({ "workspace_id": "w1" })]
    );
}

#[tokio::test]
async fn open_creates_a_workspace_in_the_repository() {
    let setup = setup();
    let opened = setup.link.open(&repos(), &"docs", "Docs").await.unwrap();
    assert_eq!(
        opened,
        Opened::Created {
            workspace_id: Some("w9".into())
        }
    );
    assert_eq!(
        setup.fake.calls("workspace.create"),
        [json!({ "cwd": "/home/dev/docs", "label": "Docs", "focus": true })]
    );
}

#[tokio::test]
async fn focuses_an_agent() {
    let setup = setup();
    setup.link.focus_agent("w1:p2").await.unwrap();
    assert_eq!(
        setup.fake.calls("agent.focus"),
        [json!({ "target": "w1:p2" })]
    );
}
