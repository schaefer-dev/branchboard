//! Getting the user's attention when an agent needs them: a system
//! notification while the window is in the background, and a sound.
//! Branchboard does both itself; the embedded herdr client is silenced (see
//! `herdr::embedded_config`), so nothing plays twice.

use std::fmt;
use std::io::Cursor;
use std::sync::mpsc;
use std::time::Duration;

use herdr::AgentState;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use workspace::Settings;

use crate::events::{ALERT_OPENED, emit};

/// Key of the agent's pane in a notification's extra payload.
const PANE_ID: &str = "paneId";

const NEEDS_INPUT_SOUND: &[u8] = include_bytes!("../sounds/needs-input.wav");
const DONE_SOUND: &[u8] = include_bytes!("../sounds/done.wav");

/// The bundled sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    NeedsInput,
    Done,
}

/// A notification about an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub pane_id: String,
    pub title: String,
    pub body: Option<String>,
}

/// Where alerts go: the system's notifications and speakers in the app, a
/// recorder in tests.
pub trait Alerts: Send + Sync + fmt::Debug + 'static {
    fn notify(&self, notice: Notice);
    fn play(&self, sound: Sound);
}

/// What the user is doing when an agent comes to need them.
#[derive(Debug, Clone, Copy)]
pub struct Situation {
    pub window_focused: bool,
    /// The window is focused and its terminal shows the agent's workspace.
    pub watching: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub notify: bool,
    pub sound: Option<Sound>,
}

/// How to alert about an agent that changed into `state`: a notification
/// only while the window is in the background (in front, the agents list
/// shows it), and a sound unless `settings` mute it.
pub fn decide(state: AgentState, situation: Situation, settings: &Settings) -> Decision {
    let sound = match state {
        AgentState::Blocked => Sound::NeedsInput,
        AgentState::Done => Sound::Done,
        AgentState::Working | AgentState::Idle | AgentState::Unknown => {
            return Decision {
                notify: false,
                sound: None,
            };
        }
    };
    let silent = settings.sound_muted || (settings.mute_when_watching && situation.watching);
    Decision {
        notify: !situation.window_focused,
        sound: (!silent).then_some(sound),
    }
}

/// e.g. "api · Claude needs input".
pub fn notice_title(place: &str, agent: &str, state: AgentState) -> String {
    let mut name = agent.to_owned();
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let doing = if state == AgentState::Blocked {
        "needs input"
    } else {
        "is done"
    };
    format!("{place} · {name} {doing}")
}

/// Shows system notifications and plays sounds on a thread of its own.
pub struct SystemAlerts<R: Runtime> {
    app: AppHandle<R>,
    sounds: mpsc::Sender<Sound>,
}

impl<R: Runtime> fmt::Debug for SystemAlerts<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemAlerts").finish_non_exhaustive()
    }
}

impl<R: Runtime> SystemAlerts<R> {
    /// Also reports clicks on notifications to the frontend, which then
    /// shows the agent.
    pub fn new(app: AppHandle<R>) -> Self {
        let (sounds, requests) = mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("sounds".into())
            .spawn(move || play_sounds(&requests))
        {
            tracing::warn!(%error, "sounds are unavailable");
        }
        let clicked = app.clone();
        let registered = app.notification().on_action(move |performed| {
            if performed.action_id() != "tap" {
                return;
            }
            let pane_id = performed
                .notification()
                .and_then(|notification| notification.extra().get(PANE_ID))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            if let Some(window) = clicked.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if let Some(pane_id) = pane_id {
                emit(&clicked, ALERT_OPENED, pane_id);
            }
        });
        if let Err(error) = registered {
            tracing::debug!(%error, "notification clicks are not reported");
        }
        Self { app, sounds }
    }
}

impl<R: Runtime> Alerts for SystemAlerts<R> {
    fn notify(&self, notice: Notice) {
        let mut builder = self
            .app
            .notification()
            .builder()
            .title(notice.title)
            .extra(PANE_ID, notice.pane_id);
        if let Some(body) = notice.body {
            builder = builder.body(body);
        }
        if let Err(error) = builder.show() {
            tracing::warn!(%error, "could not show a notification");
        }
    }

    fn play(&self, sound: Sound) {
        // Sounds play one after another; a gone thread only means silence.
        let _ = self.sounds.send(sound);
    }
}

fn play_sounds(requests: &mpsc::Receiver<Sound>) {
    for sound in requests {
        if let Err(error) = play(sound) {
            tracing::debug!(%error, "could not play a sound");
        }
    }
}

/// Opens the output device only while a sound plays, so an idle app keeps
/// no audio stream running.
fn play(sound: Sound) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = match sound {
        Sound::NeedsInput => NEEDS_INPUT_SOUND,
        Sound::Done => DONE_SOUND,
    };
    let mut device = rodio::DeviceSinkBuilder::open_default_sink()?;
    device.log_on_drop(false);
    let player = rodio::Player::connect_new(device.mixer());
    player.append(rodio::Decoder::new_wav(Cursor::new(bytes))?);
    player.sleep_until_end();
    // The device may still be draining its buffer.
    std::thread::sleep(Duration::from_millis(50));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKGROUND: Situation = Situation {
        window_focused: false,
        watching: false,
    };
    const WATCHING: Situation = Situation {
        window_focused: true,
        watching: true,
    };

    #[test]
    fn notifies_in_the_background_and_plays_the_state_sound() {
        let settings = Settings::default();
        assert_eq!(
            decide(AgentState::Blocked, BACKGROUND, &settings),
            Decision {
                notify: true,
                sound: Some(Sound::NeedsInput)
            }
        );
        assert_eq!(
            decide(AgentState::Done, BACKGROUND, &settings).sound,
            Some(Sound::Done)
        );
        let focused = Situation {
            window_focused: true,
            watching: false,
        };
        assert_eq!(
            decide(AgentState::Done, focused, &settings),
            Decision {
                notify: false,
                sound: Some(Sound::Done)
            }
        );
        assert_eq!(
            decide(AgentState::Working, BACKGROUND, &settings),
            Decision {
                notify: false,
                sound: None
            }
        );
    }

    #[test]
    fn mute_silences_sounds_but_not_notifications() {
        let muted = Settings {
            sound_muted: true,
            ..Settings::default()
        };
        assert_eq!(
            decide(AgentState::Blocked, BACKGROUND, &muted),
            Decision {
                notify: true,
                sound: None
            }
        );
        assert_eq!(decide(AgentState::Done, WATCHING, &muted).sound, None);
    }

    #[test]
    fn watching_is_silent_only_if_chosen() {
        let settings = Settings::default();
        assert!(settings.mute_when_watching);
        assert_eq!(decide(AgentState::Blocked, WATCHING, &settings).sound, None);
        let not_muted = Settings {
            mute_when_watching: false,
            ..settings
        };
        assert_eq!(
            decide(AgentState::Blocked, WATCHING, &not_muted).sound,
            Some(Sound::NeedsInput)
        );
    }

    #[test]
    fn titles_name_the_place_and_the_agent() {
        assert_eq!(
            notice_title("api", "claude", AgentState::Blocked),
            "api · Claude needs input"
        );
        assert_eq!(
            notice_title("notes", "codex", AgentState::Done),
            "notes · Codex is done"
        );
    }

    #[test]
    fn the_bundled_sounds_decode() {
        for bytes in [NEEDS_INPUT_SOUND, DONE_SOUND] {
            let decoder = rodio::Decoder::new_wav(Cursor::new(bytes)).unwrap();
            let duration = rodio::Source::total_duration(&decoder).unwrap();
            assert!(duration < Duration::from_secs(1), "{duration:?}");
        }
    }
}
