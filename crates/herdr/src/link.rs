//! The live link to a herdr session: a cached snapshot, kept current through
//! an event subscription.

use std::collections::BTreeSet;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

use serde_json::{Value, json};

use crate::connection::{self, Connection, Message};
use crate::session::SnapshotResult;
use crate::{Endpoint, HerdrError, RepoMatcher, Session};

/// Events arriving within this window cause a single snapshot.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// A steady stream of events still leads to a snapshot this often.
const MAX_BURST: Duration = Duration::from_secs(1);

/// How often to retry when panes change while subscribing.
const SUBSCRIBE_ATTEMPTS: usize = 3;

/// Events that change which panes exist or where they work. Agent state
/// changes are subscribed per pane.
const SESSION_EVENTS: &[&str] = &[
    "workspace.created",
    "workspace.closed",
    "worktree.created",
    "worktree.opened",
    "worktree.removed",
    "pane.created",
    "pane.closed",
    "pane.updated",
    "pane.exited",
    "pane.moved",
    "pane.agent_detected",
];

/// Receives the session whenever it changes.
pub trait HerdrSink: Send + Sync + 'static {
    /// The session changed, or herdr became unavailable (`None`).
    fn session_changed(&self, session: Option<Arc<Session>>);
}

/// What [`HerdrLink::open`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    Focused { workspace_id: String },
    Created { workspace_id: Option<String> },
}

/// Cheap to clone; clones share the connection.
#[derive(Clone)]
pub struct HerdrLink(Arc<Inner>);

struct Inner {
    endpoint: Option<Endpoint>,
    sink: Arc<dyn HerdrSink>,
    session: Mutex<Option<Arc<Session>>>,
    /// A subscription is being followed.
    live: AtomicBool,
    /// Serializes connection attempts.
    connecting: tokio::sync::Mutex<()>,
}

impl HerdrLink {
    /// A link to the session at `endpoint`; `None` means herdr cannot be
    /// located and the link stays unavailable.
    pub fn new(endpoint: Option<Endpoint>, sink: Arc<dyn HerdrSink>) -> Self {
        Self(Arc::new(Inner {
            endpoint,
            sink,
            session: Mutex::new(None),
            live: AtomicBool::new(false),
            connecting: tokio::sync::Mutex::new(()),
        }))
    }

    /// The last known session; `None` while herdr is unavailable.
    pub fn session(&self) -> Option<Arc<Session>> {
        self.0
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Connects unless a connection is live, and returns once the first
    /// snapshot is in or herdr turned out to be unavailable. Called on start
    /// and on window focus instead of polling for a herdr that starts later.
    pub async fn connect(&self) {
        let _attempt = self.0.connecting.lock().await;
        if self.0.live.load(Ordering::Acquire) {
            return;
        }
        let Some(endpoint) = self.0.endpoint.clone() else {
            self.0.publish(None);
            return;
        };
        match subscribe(&endpoint).await {
            Ok((subscription, session)) => {
                self.0.live.store(true, Ordering::Release);
                self.0.publish(Some(session));
                let inner = Arc::clone(&self.0);
                tokio::spawn(follow(inner, endpoint, subscription));
            }
            Err(error) => {
                tracing::debug!(%error, detail = ?error.detail(), "herdr is unavailable");
                self.0.publish(None);
            }
        }
    }

    /// Sends the current session to the sink again, e.g. after the
    /// repository list changed.
    pub fn republish(&self) {
        self.0.sink.session_changed(self.session());
    }

    /// Shows the workspace of repository `key` in herdr, or creates one in
    /// the repository named `label` if none works in it yet.
    pub async fn open<K: Clone + Eq + Hash>(
        &self,
        repos: &RepoMatcher<K>,
        key: &K,
        label: &str,
    ) -> Result<Opened, HerdrError> {
        let endpoint = self.0.endpoint.as_ref().ok_or(HerdrError::NotRunning)?;
        let session = snapshot(endpoint).await?;
        if let Some(id) = session.workspace_for(repos, key) {
            let workspace_id = id.to_owned();
            connection::request::<Value>(
                endpoint,
                "workspace.focus",
                json!({ "workspace_id": workspace_id }),
            )
            .await?;
            return Ok(Opened::Focused { workspace_id });
        }
        let cwd = repos
            .path(key)
            .ok_or_else(|| HerdrError::Protocol("unknown repository".into()))?;
        let created: Value = connection::request(
            endpoint,
            "workspace.create",
            json!({ "cwd": cwd, "label": label, "focus": true }),
        )
        .await?;
        let workspace_id = created
            .pointer("/workspace/workspace_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(Opened::Created { workspace_id })
    }
}

impl Inner {
    /// Stores `session` and tells the sink, unless nothing changed.
    fn publish(&self, session: Option<Session>) {
        let session = session.map(Arc::new);
        {
            let mut current = self.session.lock().unwrap_or_else(PoisonError::into_inner);
            if current.as_deref() == session.as_deref() {
                return;
            }
            current.clone_from(&session);
        }
        self.sink.session_changed(session);
    }
}

struct Subscription {
    connection: Connection,
    panes: BTreeSet<String>,
}

async fn snapshot(endpoint: &Endpoint) -> Result<Session, HerdrError> {
    let result: SnapshotResult =
        connection::request(endpoint, "session.snapshot", json!({})).await?;
    Ok(result.snapshot)
}

fn pane_ids(session: &Session) -> BTreeSet<String> {
    session.pane_ids().into_iter().map(str::to_owned).collect()
}

/// Subscribes to the session's events, then takes the snapshot the events
/// will invalidate (in this order, so no change falls in between).
async fn subscribe(endpoint: &Endpoint) -> Result<(Subscription, Session), HerdrError> {
    // Agent state changes can only be subscribed per pane, so the panes must
    // be known first.
    let mut panes = pane_ids(&snapshot(endpoint).await?);
    let mut attempt = 1;
    loop {
        let mut subscriptions: Vec<Value> = SESSION_EVENTS
            .iter()
            .map(|event| json!({ "type": event }))
            .collect();
        subscriptions.extend(
            panes
                .iter()
                .map(|pane| json!({ "type": "pane.agent_status_changed", "pane_id": pane })),
        );
        let mut connection = Connection::open(endpoint).await?;
        match connection
            .request::<Value>(
                "events.subscribe",
                json!({ "subscriptions": subscriptions }),
            )
            .await
        {
            Ok(_) => {}
            // A pane closed in between; herdr rejects the whole request.
            Err(HerdrError::Api { .. }) if attempt < SUBSCRIBE_ATTEMPTS => {
                attempt += 1;
                panes = pane_ids(&snapshot(endpoint).await?);
                continue;
            }
            Err(error) => return Err(error),
        }
        let session = snapshot(endpoint).await?;
        let current = pane_ids(&session);
        if current != panes && attempt < SUBSCRIBE_ATTEMPTS {
            attempt += 1;
            panes = current;
            continue;
        }
        return Ok((Subscription { connection, panes }, session));
    }
}

/// Follows a subscription until herdr goes away: every burst of events
/// leads to one new snapshot, and a changed set of panes to a new
/// subscription.
async fn follow(inner: Arc<Inner>, endpoint: Endpoint, mut subscription: Subscription) {
    loop {
        let outcome = async {
            match wait_for_events(&mut subscription.connection).await {
                Ok(()) => {}
                // herdr dropped events for a slow reader; start over.
                Err(HerdrError::Api { code, .. }) if code == "events_lost" => {
                    let (renewed, session) = subscribe(&endpoint).await?;
                    subscription = renewed;
                    return Ok(session);
                }
                Err(error) => return Err(error),
            }
            let session = snapshot(&endpoint).await?;
            if pane_ids(&session) != subscription.panes {
                let (renewed, session) = subscribe(&endpoint).await?;
                subscription = renewed;
                return Ok::<_, HerdrError>(session);
            }
            Ok(session)
        }
        .await;
        match outcome {
            Ok(session) => inner.publish(Some(session)),
            Err(error) => {
                tracing::debug!(%error, detail = ?error.detail(), "lost the connection to herdr");
                inner.live.store(false, Ordering::Release);
                inner.publish(None);
                return;
            }
        }
    }
}

/// Waits for the next event, then for the burst it starts to end (at most
/// [`MAX_BURST`]).
async fn wait_for_events(connection: &mut Connection) -> Result<(), HerdrError> {
    let mut burst_end: Option<Instant> = None;
    loop {
        let message = if let Some(end) = burst_end {
            let quiet = DEBOUNCE.min(end.saturating_duration_since(Instant::now()));
            match tokio::time::timeout(quiet, connection.receive()).await {
                Ok(message) => message?,
                Err(_) => return Ok(()),
            }
        } else {
            connection.receive().await?
        };
        match message {
            Some(Message::Event { kind }) => {
                tracing::trace!(kind, "herdr event");
                burst_end.get_or_insert_with(|| Instant::now() + MAX_BURST);
            }
            Some(Message::Response {
                outcome: Err(error),
                ..
            }) => return Err(error),
            Some(Message::Response { .. }) => {}
            None => return Err(HerdrError::Protocol("subscription closed".into())),
        }
    }
}
