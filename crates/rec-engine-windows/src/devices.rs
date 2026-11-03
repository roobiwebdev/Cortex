//! SOURCE OF TRUTH KEYWORDS: WindowFacts, is_listed_window, MonitorFacts, displays_from, window_frame, app_id, app_name, pick_camera_format, main_window
//! WHAT:  The rules that turn raw Win32 facts into the engine's device lists: which
//!        top-level windows count as capturable, how monitors become `Display`s (1-based,
//!        primary first, frames in points), where a window sits on its display, how apps
//!        are named and grouped, which window stands for an app, and which camera format
//!        to open.
//! WHY:   The UI shows the same lists on every OS (`rec_core::state`), and these rules
//!        decide what the user can pick. Kept apart from the Win32 calls so they are
//!        unit-tested on any OS.
//! WHERE: `win::devices` (the Win32 side), `win::camera`.

use std::path::Path;

use rec_core::state::{Display, Rect, display_index_for};

/// What Win32 says about one top-level window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowFacts {
    pub id: u32,
    pub title: String,
    pub pid: u32,
    /// Full path of the process image.
    pub exe: String,
    pub visible: bool,
    pub minimized: bool,
    /// DWM hides it (another virtual desktop, a suspended UWP app).
    pub cloaked: bool,
    /// `WS_EX_TOOLWINDOW` without `WS_EX_APPWINDOW`: palettes, tooltips, tray flyouts.
    pub tool: bool,
    /// Physical pixels on the virtual desktop.
    pub rect: Rect,
}

/// A window the user can pick: visible, not minimised or cloaked, titled, not a tool
/// window, not ours, and not tiny.
pub fn is_listed_window(w: &WindowFacts, our_pid: u32) -> bool {
    w.visible
        && !w.minimized
        && !w.cloaked
        && !w.tool
        && w.pid != our_pid
        && !w.title.trim().is_empty()
        && w.rect.w >= 40.0
        && w.rect.h >= 40.0
}

/// What Win32 says about one monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorFacts {
    /// Physical pixels on the virtual desktop.
    pub rect: Rect,
    /// DPI / 96.
    pub scale: f64,
    pub primary: bool,
    pub name: String,
}

/// Display order: the primary first, then left to right, top to bottom.
pub fn display_cmp(a: &MonitorFacts, b: &MonitorFacts) -> std::cmp::Ordering {
    b.primary
        .cmp(&a.primary)
        .then(a.rect.x.total_cmp(&b.rect.x))
        .then(a.rect.y.total_cmp(&b.rect.y))
}

/// Sort monitors into display order (`display_cmp`).
pub fn display_order(monitors: &mut [MonitorFacts]) {
    monitors.sort_by(display_cmp);
}

/// `Display`s for monitors already in `display_order`. Ids are 1-based. Frames are in
/// points (pixels / scale) from the primary monitor's top-left, as on macOS; with mixed
/// DPIs Windows has no single point space, so each monitor's own scale is used.
pub fn displays_from(monitors: &[MonitorFacts]) -> Vec<Display> {
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let s = if m.scale > 0.0 { m.scale } else { 1.0 };
            Display {
                id: i as u32 + 1,
                name: if m.name.is_empty() {
                    format!("Display {}", i + 1)
                } else {
                    m.name.clone()
                },
                width: m.rect.w.round().max(0.0) as u32,
                height: m.rect.h.round().max(0.0) as u32,
                frame: Rect {
                    x: m.rect.x / s,
                    y: m.rect.y / s,
                    w: m.rect.w / s,
                    h: m.rect.h / s,
                },
                scale: s,
            }
        })
        .collect()
}

/// The display (index into `monitors`) holding most of `rect` and the window's frame in
/// points from that display's top-left (what `CaptureWindow.frame` holds).
pub fn window_frame(rect: &Rect, monitors: &[MonitorFacts]) -> (usize, Rect) {
    let px: Vec<Rect> = monitors.iter().map(|m| m.rect).collect();
    let i = display_index_for(rect, &px).unwrap_or(0);
    let (origin, s) = monitors.get(i).map_or(((0.0, 0.0), 1.0), |m| {
        ((m.rect.x, m.rect.y), m.scale.max(0.1))
    });
    (
        i,
        Rect {
            x: (rect.x - origin.0) / s,
            y: (rect.y - origin.1) / s,
            w: rect.w / s,
            h: rect.h / s,
        },
    )
}

/// An app's id: its executable path in lower case (Windows paths are case-insensitive).
pub fn app_id(exe: &str) -> String {
    exe.trim().to_lowercase()
}

/// An app's name: the executable's file name without `.exe` ("chrome", "Code").
pub fn app_name(exe: &str) -> String {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// The window that stands for an app (Windows can't capture "every window of an app"):
/// its largest listed window. None when the app has none.
pub fn main_window<'a>(app: &str, windows: &'a [WindowFacts]) -> Option<&'a WindowFacts> {
    windows
        .iter()
        .filter(|w| app_id(&w.exe) == app_id(app))
        .max_by(|a, b| (a.rect.w * a.rect.h).total_cmp(&(b.rect.w * b.rect.h)))
}

/// A camera format: frame size and frame rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CamFormat {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

/// The camera format to open, as `index` into `formats`: the largest at or under
/// `max_height` that runs at 24 fps or faster (the macOS `camera::pick_format` rule), else
/// the smallest one, else None.
pub fn pick_camera_format(formats: &[CamFormat], max_height: u32) -> Option<usize> {
    let pixels = |f: &CamFormat| f.width as u64 * f.height as u64;
    formats
        .iter()
        .enumerate()
        .filter(|(_, f)| f.height <= max_height && f.fps >= 23.9)
        .max_by(|(_, a), (_, b)| pixels(a).cmp(&pixels(b)).then(a.fps.total_cmp(&b.fps)))
        .or_else(|| formats.iter().enumerate().min_by_key(|(_, f)| pixels(f)))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn only_real_windows_are_listed() {
        let ok = WindowFacts {
            id: 1,
            title: "Doc".into(),
            pid: 10,
            exe: r"C:\a\b.exe".into(),
            visible: true,
            rect: r(0.0, 0.0, 800.0, 600.0),
            ..WindowFacts::default()
        };
        assert!(is_listed_window(&ok, 1));
        assert!(!is_listed_window(&ok, 10), "ours");
        for bad in [
            WindowFacts {
                visible: false,
                ..ok.clone()
            },
            WindowFacts {
                minimized: true,
                ..ok.clone()
            },
            WindowFacts {
                cloaked: true,
                ..ok.clone()
            },
            WindowFacts {
                tool: true,
                ..ok.clone()
            },
            WindowFacts {
                title: " ".into(),
                ..ok.clone()
            },
            WindowFacts {
                rect: r(0.0, 0.0, 10.0, 600.0),
                ..ok.clone()
            },
        ] {
            assert!(!is_listed_window(&bad, 1), "{bad:?}");
        }
    }

    #[test]
    fn displays_are_primary_first_in_points() {
        let mut m = vec![
            MonitorFacts {
                rect: r(-1920.0, 0.0, 1920.0, 1080.0),
                scale: 1.0,
                primary: false,
                name: String::new(),
            },
            MonitorFacts {
                rect: r(0.0, 0.0, 3840.0, 2160.0),
                scale: 1.5,
                primary: true,
                name: "DELL U2720Q".into(),
            },
        ];
        display_order(&mut m);
        let d = displays_from(&m);
        assert_eq!(d[0].id, 1);
        assert_eq!(d[0].name, "DELL U2720Q");
        assert_eq!((d[0].width, d[0].height, d[0].scale), (3840, 2160, 1.5));
        assert_eq!(d[0].frame, r(0.0, 0.0, 2560.0, 1440.0));
        assert_eq!(d[1].name, "Display 2");
        assert_eq!(d[1].frame, r(-1920.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn windows_sit_on_the_display_holding_most_of_them() {
        let m = vec![
            MonitorFacts {
                rect: r(0.0, 0.0, 3840.0, 2160.0),
                scale: 2.0,
                primary: true,
                name: String::new(),
            },
            MonitorFacts {
                rect: r(3840.0, 0.0, 1920.0, 1080.0),
                scale: 1.0,
                primary: false,
                name: String::new(),
            },
        ];
        let (i, f) = window_frame(&r(200.0, 100.0, 800.0, 600.0), &m);
        assert_eq!((i, f), (0, r(100.0, 50.0, 400.0, 300.0)));
        let (i, f) = window_frame(&r(4000.0, 10.0, 800.0, 600.0), &m);
        assert_eq!((i, f), (1, r(160.0, 10.0, 800.0, 600.0)));
    }

    #[test]
    fn apps_are_named_by_their_executable() {
        assert_eq!(
            app_name(r"C:\Program Files\Google\Chrome\chrome.exe"),
            "chrome"
        );
        assert_eq!(app_name("Code.exe"), "Code");
        assert_eq!(app_id(r"C:\X\Code.EXE"), r"c:\x\code.exe");
    }

    #[test]
    fn an_app_is_its_largest_window() {
        let w = |id, exe: &str, a: f64| WindowFacts {
            id,
            exe: exe.into(),
            rect: r(0.0, 0.0, a, a),
            ..WindowFacts::default()
        };
        let ws = [
            w(1, r"C:\a.exe", 100.0),
            w(2, r"C:\A.EXE", 300.0),
            w(3, r"C:\b.exe", 900.0),
        ];
        assert_eq!(main_window(r"c:\a.exe", &ws).map(|w| w.id), Some(2));
        assert_eq!(main_window(r"c:\z.exe", &ws), None);
    }

    #[test]
    fn camera_formats_follow_the_cap() {
        let f = |width, height, fps| CamFormat { width, height, fps };
        let formats = [
            f(640, 480, 30.0),
            f(1280, 720, 30.0),
            f(1920, 1080, 30.0),
            f(1920, 1080, 5.0),
            f(3840, 2160, 30.0),
        ];
        assert_eq!(pick_camera_format(&formats, 1080), Some(2));
        assert_eq!(pick_camera_format(&formats, 720), Some(1));
        // Nothing fits: the smallest.
        assert_eq!(pick_camera_format(&formats, 240), Some(0));
        assert_eq!(pick_camera_format(&[], 720), None);
    }
}
