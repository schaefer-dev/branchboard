//! Client for a running [herdr](https://herdr.dev) session: which coding
//! agents run in which repository, and opening a repository's workspace.
//!
//! Talks to herdr's socket API (newline-delimited JSON over a Unix socket or
//! a Windows named pipe) and parses only the fields it needs, so newer herdr
//! versions with additional fields keep working. Independent of Tauri;
//! changes leave through [`HerdrSink`].

mod connection;
mod endpoint;
mod error;
mod link;
mod repos;
mod session;

pub use endpoint::Endpoint;
pub use error::HerdrError;
pub use link::{HerdrLink, HerdrSink, Opened};
pub use repos::{AgentSummary, RepoAgents, RepoMatcher};
pub use session::{AgentState, Session};
