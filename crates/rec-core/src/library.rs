//! SOURCE OF TRUTH KEYWORDS: recordings library, TrimRequest, TrimMode, TrimOutput, KeepTracks, TrimProgress, validate_name, trim_range, unique_name
//! WHAT:  The platform-neutral rules of the recordings library and Quick trim: which files count
//!        as recordings, safe file names for rename, the trim request and its validation, and
//!        the "<name>-trim.<ext>" copy name that never overwrites a file (NEW-TR-06).
//! WHY:   The webview sends names and times; Rust checks them here before any file is touched,
//!        and the rules are pure so they are unit-tested without AVFoundation.
//! WHERE: the Tauri shell's `trim.rs` (commands) and the macOS engine's `trim.rs` (export).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::api::EngineError;

/// File extensions the recordings list shows (lower case).
pub const RECORDING_EXTS: [&str; 2] = ["mov", "mp4"];
/// The list shows at most this many recordings, newest first.
pub const MAX_RECORDINGS: usize = 200;
/// Shortest selection a trim may keep, in seconds.
pub const MIN_TRIM_SECS: f64 = 0.1;
/// Longest file name (without extension) a rename accepts, in characters.
pub const MAX_NAME_CHARS: usize = 180;
/// Bounds for the timeline's thumbnail and waveform requests.
pub const MAX_THUMBNAILS: u32 = 64;
pub const MAX_WAVEFORM_BUCKETS: u32 = 4000;

/// Fast copies the original samples (passthrough, edit list); Exact re-encodes the video.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum TrimMode {
    #[default]
    Fast,
    Exact,
}

/// Copy writes "<name>-trim.<ext>"; Replace moves the original to the Trash and takes its name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum TrimOutput {
    #[default]
    Copy,
    Replace,
}

/// Which optional tracks the trimmed file keeps. The screen track is always kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct KeepTracks {
    pub camera: bool,
    pub system: bool,
    pub mic: bool,
}

impl Default for KeepTracks {
    fn default() -> Self {
        KeepTracks {
            camera: true,
            system: true,
            mic: true,
        }
    }
}

/// `trim_recording`'s options, in seconds of the source file.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TrimRequest {
    pub start_secs: f64,
    pub end_secs: f64,
    pub mode: TrimMode,
    pub output: TrimOutput,
    pub keep: KeepTracks,
}

/// Export progress for the recording `id`, 0..1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TrimProgress {
    pub id: String,
    pub progress: f64,
}

/// True for a regular-looking recording file name: a known extension, not hidden.
pub fn is_recording_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RECORDING_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// A new file name (without extension) from the user, trimmed, or why it is refused.
/// Refuses anything that could leave the folder or confuse Finder: separators, `:`,
/// control characters, a leading dot, and over-long names.
pub fn validate_name(name: &str) -> Result<String, EngineError> {
    let name = name.trim();
    let bad = |why: &str| Err(EngineError::failed(why.to_string()));
    if name.is_empty() {
        return bad("The name can't be empty");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return bad("That name is too long");
    }
    if name.starts_with('.') {
        return bad("The name can't start with a dot");
    }
    if name
        .chars()
        .any(|c| c == '/' || c == '\\' || c == ':' || c.is_control())
    {
        return bad("The name can't contain / \\ : or control characters");
    }
    Ok(name.to_string())
}

/// The selection to keep, clamped to the file: `end` past the end (by rounding) becomes the
/// duration. Errors when the numbers are not finite, out of order or too short.
pub fn trim_range(start: f64, end: f64, duration: f64) -> Result<(f64, f64), EngineError> {
    if !(start.is_finite() && end.is_finite() && duration.is_finite()) || duration <= 0.0 {
        return Err(EngineError::failed("The trim times are not valid"));
    }
    let start = start.max(0.0);
    let end = end.min(duration);
    if start >= duration || end - start < MIN_TRIM_SECS {
        return Err(EngineError::failed("The selection is too short to export"));
    }
    Ok((start, end))
}

/// `<stem><suffix>.<ext>`, then `<stem><suffix> 2.<ext>`, 3, ... until `exists` says the
/// name is free. The trimmed copy never overwrites a file.
pub fn unique_name(stem: &str, suffix: &str, ext: &str, exists: impl Fn(&str) -> bool) -> String {
    let make = |n: u32| {
        if n <= 1 {
            format!("{stem}{suffix}.{ext}")
        } else {
            format!("{stem}{suffix} {n}.{ext}")
        }
    };
    let mut n = 1;
    loop {
        let name = make(n);
        if !exists(&name) || n >= 10_000 {
            return name;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_files() {
        assert!(is_recording_file(Path::new("/x/Cortex 1.mov")));
        assert!(is_recording_file(Path::new("/x/a.MP4")));
        assert!(!is_recording_file(Path::new("/x/.a.trimming.mov")));
        assert!(!is_recording_file(Path::new("/x/a.mkv")));
        assert!(!is_recording_file(Path::new("/x/mov")));
    }

    #[test]
    fn names() {
        assert_eq!(validate_name("  Demo take 2 ").unwrap(), "Demo take 2");
        assert_eq!(validate_name("Café – été").unwrap(), "Café – été");
        for bad in ["", "   ", "../x", "a/b", "a:b", ".hidden", "a\nb", "a\\b"] {
            assert!(validate_name(bad).is_err(), "{bad:?} accepted");
        }
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn ranges() {
        assert_eq!(trim_range(1.0, 4.0, 6.0).unwrap(), (1.0, 4.0));
        assert_eq!(trim_range(-0.2, 6.01, 6.0).unwrap(), (0.0, 6.0));
        assert!(trim_range(4.0, 1.0, 6.0).is_err());
        assert!(trim_range(1.0, 1.05, 6.0).is_err());
        assert!(trim_range(6.5, 7.0, 6.0).is_err());
        assert!(trim_range(f64::NAN, 1.0, 6.0).is_err());
        assert!(trim_range(0.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn unique_names() {
        let taken = ["a-trim.mov", "a-trim 2.mov"];
        assert_eq!(
            unique_name("b", "-trim", "mov", |n| taken.contains(&n)),
            "b-trim.mov"
        );
        assert_eq!(
            unique_name("a", "-trim", "mov", |n| taken.contains(&n)),
            "a-trim 3.mov"
        );
        assert_eq!(unique_name("a", "", "mp4", |_| false), "a.mp4");
    }

    #[test]
    fn request_json() {
        let r: TrimRequest = serde_json::from_str(
            r#"{"startSecs":1,"endSecs":4,"mode":"exact","output":"replace","keep":{"camera":false,"system":true,"mic":true}}"#,
        )
        .unwrap();
        assert_eq!(r.mode, TrimMode::Exact);
        assert_eq!(r.output, TrimOutput::Replace);
        assert!(!r.keep.camera);
    }
}
