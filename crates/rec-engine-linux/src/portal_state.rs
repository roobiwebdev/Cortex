//! SOURCE OF TRUTH KEYWORDS: PortalState, restore token, persist_mode, screencast.json, last stream size
//! WHAT:  What the ScreenCast portal lets us keep between launches: one restore token per
//!        kind of pick (a screen, a window) and the last stream's size, in
//!        `$XDG_STATE_HOME/cortex/screencast.json` (`~/.local/state/cortex/…`).
//! WHY:   With a restore token (`persist_mode` 2) the portal reuses the user's last choice
//!        instead of showing its picker every time. The token is opaque, per machine and
//!        rewritten on every use, so it is engine state, not a user setting: it stays out of
//!        `RecorderSettings` (synced to the UI, exported, imported) and has its own file.
//!        Screen and window picks keep separate tokens so switching the target in Cortex
//!        does not silently restore the other kind.
//! WHERE: `portal` (read before a pick, written after), `engine` (display size).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Which portal source types a pick asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    Monitor,
    Window,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalState {
    pub monitor_token: Option<String>,
    pub window_token: Option<String>,
    /// The last stream's size in pixels, so the UI can show the shape before a recording.
    pub last_size: Option<(u32, u32)>,
}

impl PortalState {
    pub fn token(&self, kind: PickKind) -> Option<&str> {
        match kind {
            PickKind::Monitor => self.monitor_token.as_deref(),
            PickKind::Window => self.window_token.as_deref(),
        }
    }

    pub fn set_token(&mut self, kind: PickKind, token: Option<String>) {
        match kind {
            PickKind::Monitor => self.monitor_token = token,
            PickKind::Window => self.window_token = token,
        }
    }

    /// The state file under `home` (or `$XDG_STATE_HOME` when set).
    pub fn path(home: &Path, xdg_state_home: Option<&str>) -> PathBuf {
        let base = xdg_state_home
            .filter(|s| s.starts_with('/'))
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        base.join("cortex/screencast.json")
    }

    /// Defaults when the file is missing or unreadable (the portal then asks again).
    pub fn load(path: &Path) -> PortalState {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Written through a temporary file so a crash never leaves half a token.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_kept_per_kind_and_survive_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("cortex-portal-{}", std::process::id()));
        let path = PortalState::path(&dir, None);
        assert!(path.ends_with(".local/state/cortex/screencast.json"));
        assert_eq!(PortalState::load(&path), PortalState::default());
        let mut s = PortalState::default();
        s.set_token(PickKind::Monitor, Some("m-1".into()));
        s.set_token(PickKind::Window, Some("w-1".into()));
        s.last_size = Some((2560, 1440));
        s.save(&path).unwrap();
        let back = PortalState::load(&path);
        assert_eq!(back, s);
        assert_eq!(back.token(PickKind::Monitor), Some("m-1"));
        assert_eq!(back.token(PickKind::Window), Some("w-1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn xdg_state_home_wins_when_absolute() {
        let home = Path::new("/home/u");
        assert_eq!(
            PortalState::path(home, Some("/var/state")),
            Path::new("/var/state/cortex/screencast.json")
        );
        assert_eq!(
            PortalState::path(home, Some("relative")),
            Path::new("/home/u/.local/state/cortex/screencast.json")
        );
    }

    #[test]
    fn a_corrupt_file_reads_as_empty() {
        let dir = std::env::temp_dir().join(format!("cortex-portal-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("screencast.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(PortalState::load(&path), PortalState::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
