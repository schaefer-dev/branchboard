//! Client for a running [herdr](https://herdr.dev) session: which coding
//! agents run in which repository, and opening a repository's workspace.
//!
//! Talks to herdr's socket API (newline-delimited JSON over a Unix socket or
//! a Windows named pipe) and parses only the fields it needs, so newer herdr
//! versions with additional fields keep working. Independent of Tauri;
//! changes leave through [`HerdrSink`].
//!
//! # Tested herdr version
//!
//! Implemented and tested against **herdr 0.9.3** (socket API protocol 22,
//! schema version 1); see [`TESTED_VERSION`] and [`TESTED_PROTOCOL`]. When the
//! integration breaks after a herdr update, compare herdr's changelog and
//! `herdr api schema --json` from that version with the methods, fields and
//! events used here:
//!
//! - requests: `session.snapshot`, `events.subscribe`, `workspace.focus`,
//!   `workspace.create`, `agent.focus`;
//! - snapshot fields: see `session.rs`;
//! - events: see `SESSION_EVENTS` and `pane.agent_status_changed` in
//!   `link.rs`.
//!
//! A server reporting a different protocol is logged on connect.

mod connection;
mod endpoint;
mod error;
mod link;
mod repos;
mod session;

pub use endpoint::Endpoint;
pub use error::HerdrError;
pub use link::{HerdrLink, HerdrSink, Opened};
pub use repos::{AgentEntry, AgentSummary, RepoAgents, RepoMatcher};
pub use session::{AgentState, Session};

/// The herdr release this integration was implemented and tested with.
pub const TESTED_VERSION: &str = "0.9.3";

/// The socket API protocol version of [`TESTED_VERSION`], as reported by
/// `ping` and `session.snapshot`.
pub const TESTED_PROTOCOL: u32 = 22;
