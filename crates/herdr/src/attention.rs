//! Which agents just came to need the user: they became blocked (waiting
//! for input) or done (finished, not yet seen).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{AgentState, Session};

/// How long a new state must hold before it counts, like herdr's own
/// `ui.toast.delay_seconds`: an agent that asks and continues right away,
/// or finishes while the user watches, raises no alert.
pub const ALERT_DELAY: Duration = Duration::from_secs(1);

/// An agent that came to need the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub pane_id: String,
    /// [`AgentState::Blocked`] or [`AgentState::Done`].
    pub state: AgentState,
}

/// Follows the agent states of consecutive sessions and reports changes into
/// blocked or done once they held for the delay.
#[derive(Debug)]
pub struct Attention {
    delay: Duration,
    /// The states of the last session; `None` until the first session after
    /// herdr became available, which only sets the baseline.
    known: Option<HashMap<String, AgentState>>,
    pending: HashMap<String, Pending>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    state: AgentState,
    due: Instant,
}

impl Attention {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            known: None,
            pending: HashMap::new(),
        }
    }

    /// Takes the current session (`None` while herdr is unavailable) and
    /// returns the agents whose new state is due at `now`. Call it again
    /// at [`Self::next_due`] with the session of that time.
    pub fn observe(&mut self, session: Option<&Session>, now: Instant) -> Vec<Alert> {
        let Some(session) = session else {
            // A reconnect starts over, without alerts for what happened meanwhile.
            self.known = None;
            self.pending.clear();
            return Vec::new();
        };
        let current: HashMap<String, AgentState> = session
            .agents
            .iter()
            .map(|agent| (agent.pane_id.clone(), agent.agent_status))
            .collect();
        if let Some(known) = &self.known {
            for (pane_id, &state) in &current {
                let alerting = matches!(state, AgentState::Blocked | AgentState::Done);
                if alerting && known.get(pane_id) != Some(&state) {
                    self.pending.insert(
                        pane_id.clone(),
                        Pending {
                            state,
                            due: now + self.delay,
                        },
                    );
                }
            }
        }
        // A state that did not hold (or an agent that went away) is dropped.
        self.pending
            .retain(|pane_id, pending| current.get(pane_id) == Some(&pending.state));
        let mut due: Vec<Alert> = Vec::new();
        self.pending.retain(|pane_id, pending| {
            if pending.due > now {
                return true;
            }
            due.push(Alert {
                pane_id: pane_id.clone(),
                state: pending.state,
            });
            false
        });
        due.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
        self.known = Some(current);
        due
    }

    /// When the next pending state becomes due, if any.
    pub fn next_due(&self) -> Option<Instant> {
        self.pending.values().map(|pending| pending.due).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(agents: &[(&str, &str)]) -> Session {
        let agents: Vec<serde_json::Value> = agents
            .iter()
            .map(|(pane_id, status)| {
                serde_json::json!({ "pane_id": pane_id, "workspace_id": "w1", "agent_status": status })
            })
            .collect();
        serde_json::from_value(serde_json::json!({ "agents": agents })).unwrap()
    }

    fn alert(pane_id: &str, state: AgentState) -> Alert {
        Alert {
            pane_id: pane_id.to_owned(),
            state,
        }
    }

    const DELAY: Duration = Duration::from_secs(1);

    #[test]
    fn the_first_session_only_sets_the_baseline() {
        let start = Instant::now();
        let mut attention = Attention::new(DELAY);
        let first = session(&[("p1", "blocked"), ("p2", "done")]);
        assert_eq!(attention.observe(Some(&first), start), []);
        assert_eq!(attention.observe(Some(&first), start + DELAY * 2), []);
        assert_eq!(attention.next_due(), None);
    }

    #[test]
    fn reports_a_change_into_blocked_or_done_once_it_held() {
        let start = Instant::now();
        let mut attention = Attention::new(DELAY);
        attention.observe(
            Some(&session(&[("p1", "working"), ("p2", "working")])),
            start,
        );

        let changed = session(&[("p1", "blocked"), ("p2", "done")]);
        assert_eq!(attention.observe(Some(&changed), start), []);
        assert_eq!(attention.next_due(), Some(start + DELAY));
        assert_eq!(
            attention.observe(Some(&changed), start + DELAY),
            [
                alert("p1", AgentState::Blocked),
                alert("p2", AgentState::Done)
            ]
        );
        // Reported once, however often the same state is seen again.
        assert_eq!(attention.observe(Some(&changed), start + DELAY * 3), []);
        assert_eq!(attention.next_due(), None);
    }

    #[test]
    fn a_state_that_did_not_hold_is_not_reported() {
        let start = Instant::now();
        let mut attention = Attention::new(DELAY);
        attention.observe(Some(&session(&[("p1", "working")])), start);
        attention.observe(Some(&session(&[("p1", "blocked")])), start);
        // Answered (or seen) within the delay.
        attention.observe(Some(&session(&[("p1", "working")])), start + DELAY / 2);
        assert_eq!(
            attention.observe(Some(&session(&[("p1", "working")])), start + DELAY),
            []
        );
        // Closed within the delay.
        attention.observe(Some(&session(&[("p1", "done")])), start + DELAY);
        assert_eq!(
            attention.observe(Some(&session(&[])), start + DELAY * 3),
            []
        );
    }

    #[test]
    fn blocked_after_done_and_new_agents_count() {
        let start = Instant::now();
        let mut attention = Attention::new(Duration::ZERO);
        attention.observe(Some(&session(&[("p1", "done")])), start);
        assert_eq!(
            attention.observe(
                Some(&session(&[("p1", "blocked"), ("p2", "blocked")])),
                start
            ),
            [
                alert("p1", AgentState::Blocked),
                alert("p2", AgentState::Blocked)
            ]
        );
        assert_eq!(
            attention.observe(Some(&session(&[("p1", "idle"), ("p2", "idle")])), start),
            []
        );
    }

    #[test]
    fn herdr_going_away_starts_over() {
        let start = Instant::now();
        let mut attention = Attention::new(DELAY);
        attention.observe(Some(&session(&[("p1", "working")])), start);
        attention.observe(Some(&session(&[("p1", "blocked")])), start);
        assert_eq!(attention.observe(None, start), []);
        assert_eq!(attention.next_due(), None);
        // The first session after the reconnect is a new baseline.
        assert_eq!(
            attention.observe(Some(&session(&[("p1", "blocked")])), start + DELAY * 2),
            []
        );
    }
}
