//! SOURCE OF TRUTH KEYWORDS: RecorderEngine, EngineEvent, EngineError, Action, transition, state machine
//! WHAT:  The platform-neutral engine API: commands (the `RecorderEngine` trait), events, errors,
//!        and the recording state machine (idle → countdown → recording ⇄ paused → finalizing → idle).
//! WHY:   The Tauri shell, the CLI control socket and tests drive any engine (macOS today,
//!        Windows later) through this one surface. The state machine is pure so it is unit
//!        tested here and every engine enforces the same legal transitions.
//! WHERE: `rec-engine-macos::engine::MacEngine` implements it; the Tauri commands call it.

use serde::{Deserialize, Serialize};

use crate::settings::RecorderSettings;
use crate::state::{
    Devices, EngineState, EngineStatus, Levels, Permission, PermissionState, Progress, Recording,
};

/// Something the engine was asked to do (or that happened to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Start with a countdown first (`countdown > 0`).
    StartCountdown,
    /// Start recording now (countdown 0, or the countdown ran out).
    Start,
    CancelCountdown,
    Pause,
    Resume,
    /// Stop and keep the file.
    Stop,
    /// Stop and delete the file (or cancel the countdown).
    Discard,
    /// A source or the writer failed mid-recording: finalize what was written.
    Fail,
    /// The file is written (or deleted after a discard).
    Finalized,
}

/// The status after `action` in `from`, or why it isn't allowed.
pub fn transition(from: EngineStatus, action: Action) -> Result<EngineStatus, EngineError> {
    use Action as A;
    use EngineStatus as S;
    let to = match (from, action) {
        (S::Idle, A::StartCountdown) => S::Countdown,
        (S::Idle | S::Countdown, A::Start) => S::Recording,
        (S::Countdown, A::CancelCountdown | A::Discard) => S::Idle,
        (S::Recording, A::Pause) => S::Paused,
        (S::Paused, A::Resume) => S::Recording,
        (S::Recording | S::Paused, A::Stop | A::Discard | A::Fail) => S::Finalizing,
        (S::Finalizing, A::Finalized) => S::Idle,
        _ => {
            return Err(EngineError::InvalidState {
                status: from,
                action,
            });
        }
    };
    Ok(to)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EngineError {
    /// `action` isn't allowed in `status` (e.g. pause while idle).
    InvalidState {
        status: EngineStatus,
        action: Action,
    },
    /// Privacy grants the session needs and doesn't have.
    MissingPermissions { permissions: Vec<Permission> },
    /// A setting asks for something this engine can't do yet.
    Unsupported { message: String },
    /// Anything else (device, encoder, disk, network); the message is user-facing.
    Failed { message: String },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::InvalidState { status, action } => {
                write!(f, "can't {action:?} while {status:?}")
            }
            EngineError::MissingPermissions { permissions } => {
                write!(f, "missing permission(s): {permissions:?}")
            }
            EngineError::Unsupported { message } | EngineError::Failed { message } => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for EngineError {}

impl EngineError {
    pub fn failed(message: impl Into<String>) -> Self {
        EngineError::Failed {
            message: message.into(),
        }
    }
}

/// What the engine tells its listener. Levels come at most 30 times a second,
/// progress at most 10; the rest when they happen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "data", rename_all = "kebab-case")]
pub enum EngineEvent {
    /// The status (or anything other than progress/levels) changed.
    StateChanged(EngineState),
    Progress(Progress),
    Levels(Levels),
    RecordingFinished {
        recording: Recording,
    },
    ReplaySaved {
        path: String,
    },
    /// `recoverable` false: the recording or preview stopped because of it.
    Error {
        message: String,
        recoverable: bool,
    },
    DeviceChanged(Devices),
    PermissionChanged {
        permission: Permission,
        state: PermissionState,
    },
}

/// Where events go: any thread, so keep it quick (forward to a channel or the webview).
pub type EventSink = std::sync::Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// The recorder. Calls may come from any thread; the engine serializes them.
/// Preview frames are platform-specific (a CVPixelBuffer on macOS), so the
/// preview sink is set on the platform engine, not here.
pub trait RecorderEngine: Send + Sync {
    /// The current state (also sent as `StateChanged` events).
    fn state(&self) -> EngineState;
    /// Start recording with these settings (after `settings.countdown` seconds).
    fn start(&self, settings: &RecorderSettings) -> Result<(), EngineError>;
    fn cancel_countdown(&self) -> Result<(), EngineError>;
    /// Stop and keep the file; `RecordingFinished` follows once it is written.
    fn stop(&self) -> Result<(), EngineError>;
    fn pause(&self) -> Result<(), EngineError>;
    fn resume(&self) -> Result<(), EngineError>;
    /// Stop without keeping the file (or cancel the countdown).
    fn discard(&self) -> Result<(), EngineError>;
    /// Apply changed settings: overlay and fader changes apply live; capture
    /// changes restart the preview (never a running recording).
    fn update_settings(&self, settings: &RecorderSettings) -> Result<(), EngineError>;
    /// Run (or stop) a preview session while nothing records.
    fn set_preview_enabled(&self, enabled: bool) -> Result<(), EngineError>;
    /// Write the replay buffer to a file; `ReplaySaved` follows.
    fn save_replay(&self) -> Result<(), EngineError>;
    /// Stream to every enabled destination (all or nothing).
    fn go_live(&self) -> Result<(), EngineError>;
    fn stop_live(&self) -> Result<(), EngineError>;
    fn devices(&self) -> Devices;
    fn permission(&self, p: Permission) -> PermissionState;
    /// Ask the user (blocks until answered or a timeout); returns the new state.
    fn request_permission(&self, p: Permission) -> PermissionState;
    /// Finalize anything running (quit). Blocks until files are written.
    fn shutdown(&self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action as A;
    use EngineStatus as S;

    #[test]
    fn the_happy_paths() {
        let run = |actions: &[Action]| actions.iter().try_fold(S::Idle, |s, a| transition(s, *a));
        assert_eq!(
            run(&[
                A::StartCountdown,
                A::Start,
                A::Pause,
                A::Resume,
                A::Stop,
                A::Finalized
            ]),
            Ok(S::Idle)
        );
        assert_eq!(run(&[A::Start, A::Stop]), Ok(S::Finalizing));
        assert_eq!(
            run(&[A::Start, A::Pause, A::Stop, A::Finalized]),
            Ok(S::Idle)
        );
        assert_eq!(run(&[A::StartCountdown, A::CancelCountdown]), Ok(S::Idle));
        assert_eq!(run(&[A::StartCountdown, A::Discard]), Ok(S::Idle));
        assert_eq!(
            run(&[A::Start, A::Pause, A::Discard, A::Finalized]),
            Ok(S::Idle)
        );
        assert_eq!(run(&[A::Start, A::Fail, A::Finalized]), Ok(S::Idle));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let bad = [
            (S::Idle, A::Pause),
            (S::Idle, A::Stop),
            (S::Idle, A::Resume),
            (S::Idle, A::Discard),
            (S::Idle, A::Finalized),
            (S::Countdown, A::Pause),
            (S::Countdown, A::Stop),
            (S::Countdown, A::StartCountdown),
            (S::Recording, A::Start),
            (S::Recording, A::Resume),
            (S::Recording, A::CancelCountdown),
            (S::Paused, A::Pause),
            (S::Paused, A::Start),
            (S::Finalizing, A::Start),
            (S::Finalizing, A::Stop),
            (S::Finalizing, A::Pause),
        ];
        for (s, a) in bad {
            assert_eq!(
                transition(s, a),
                Err(EngineError::InvalidState {
                    status: s,
                    action: a
                }),
                "{s:?} {a:?}"
            );
        }
    }

    #[test]
    fn events_and_errors_serialize_tagged() {
        let e = EngineEvent::Error {
            message: "disk full".into(),
            recoverable: false,
        };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"type": "error", "data": {"message": "disk full", "recoverable": false}})
        );
        let err = transition(S::Idle, A::Pause).unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            serde_json::json!({"kind": "invalid-state", "status": "idle", "action": "pause"})
        );
        assert_eq!(err.to_string(), "can't Pause while Idle");
    }
}
