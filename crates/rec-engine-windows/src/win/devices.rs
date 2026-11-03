//! SOURCE OF TRUTH KEYWORDS: monitors, list_windows, list_devices, EnumDisplayMonitors, EnumWindows, DWMWA_CLOAKED, own windows, permissions, consent store, HWND id
//! WHAT:  The Win32 side of the device lists: monitors (with DPI scale), top-level windows
//!        (title, process, visibility, cloaking, frame), our own windows, and the camera
//!        and microphone privacy switches. `list_devices` builds the engine's `Devices`.
//! WHY:   The pure rules in `crate::devices` decide what is listed; this file only asks
//!        Windows. Window ids are HWNDs as u32: Windows keeps window handles within 32 bits
//!        so 32- and 64-bit processes can share them, which makes the round trip safe.
//! WHERE: `WinBackend::devices`, `win::session` (targets), `win::shell` (own windows).

use rec_core::state::{
    CaptureWindow, Devices, Permission, PermissionState, Permissions, Rect, apps_from_windows,
};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, MONITORINFOF_PRIMARY, WS_EX_APPWINDOW,
    WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

use super::util::{from_wide, reg_string};
use crate::caps::consent_state;
use crate::devices::{
    MonitorFacts, WindowFacts, app_id, app_name, display_cmp, displays_from, is_listed_window,
    window_frame,
};

/// A monitor with its handle, in display order.
#[derive(Clone, Debug)]
pub struct Monitor {
    pub handle: HMONITOR,
    pub facts: MonitorFacts,
}

// SAFETY: an HMONITOR is a plain identifier, valid from any thread.
unsafe impl Send for Monitor {}

fn rect(r: &RECT) -> Rect {
    Rect {
        x: f64::from(r.left),
        y: f64::from(r.top),
        w: f64::from(r.right - r.left),
        h: f64::from(r.bottom - r.top),
    }
}

unsafe extern "system" fn on_monitor(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the `Vec<HMONITOR>` passed by `monitors`, alive for the enumeration.
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    list.push(m);
    true.into()
}

/// Monitors, primary first (`crate::devices::display_order`); index + 1 is the display id.
pub fn monitors() -> Vec<Monitor> {
    let mut handles: Vec<HMONITOR> = vec![];
    // SAFETY: the callback only pushes into `handles`, which outlives the call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(on_monitor),
            LPARAM(&mut handles as *mut _ as isize),
        )
    };
    let mut out: Vec<Monitor> = handles
        .into_iter()
        .filter_map(|h| {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            // SAFETY: `info` is a MONITORINFOEXW with its size set, as the API requires.
            let ok = unsafe {
                GetMonitorInfoW(h, (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>())
            };
            if !ok.as_bool() {
                return None;
            }
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: out-pointers are valid.
            let _ = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            Some(Monitor {
                handle: h,
                facts: MonitorFacts {
                    rect: rect(&info.monitorInfo.rcMonitor),
                    scale: f64::from(dx.max(1)) / 96.0,
                    primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                    name: String::new(),
                },
            })
        })
        .collect();
    out.sort_by(|a, b| display_cmp(&a.facts, &b.facts));
    out
}

/// The executable path of a process, or empty.
fn process_exe(pid: u32) -> String {
    // SAFETY: the handle is opened with query rights only and closed below; the buffer
    // length is passed in and updated by the call.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let r =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        if r.is_ok() {
            from_wide(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

/// A window id (`CaptureWindow.id`) as a handle. Windows widens 32-bit handles by sign
/// extension, so the id goes back through `i32`.
pub fn hwnd(id: u32) -> HWND {
    HWND(id as i32 as isize as *mut core::ffi::c_void)
}

fn window_id(h: HWND) -> u32 {
    h.0 as usize as u32
}

unsafe extern "system" fn on_window(h: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the `Vec<HWND>` passed by `top_level_windows`.
    let list = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
    list.push(h);
    true.into()
}

/// Every top-level window, front to back.
pub fn top_level_windows() -> Vec<HWND> {
    let mut list: Vec<HWND> = vec![];
    // SAFETY: the callback only pushes into `list`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(on_window), LPARAM(&mut list as *mut _ as isize)) };
    list
}

/// What Win32 says about a window.
pub fn window_facts(h: HWND) -> WindowFacts {
    // SAFETY: read-only queries on a window handle; a closed window just fails them.
    unsafe {
        let len = GetWindowTextLengthW(h).max(0) as usize;
        let mut title = vec![0u16; len + 1];
        let n = GetWindowTextW(h, &mut title).max(0) as usize;
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
        // The visible frame (without the invisible resize borders), else the window rect.
        let mut r = RECT::default();
        if DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            let _ = GetWindowRect(h, &mut r);
        }
        WindowFacts {
            id: window_id(h),
            title: String::from_utf16_lossy(&title[..n.min(len)]),
            pid,
            exe: String::new(),
            visible: IsWindowVisible(h).as_bool(),
            minimized: IsIconic(h).as_bool(),
            cloaked: cloaked != 0,
            tool: ex & WS_EX_TOOLWINDOW.0 != 0 && ex & WS_EX_APPWINDOW.0 == 0,
            rect: rect(&r),
        }
    }
}

fn our_pid() -> u32 {
    // SAFETY: no arguments.
    unsafe { GetCurrentProcessId() }
}

/// The windows the user can pick, with their executables.
pub fn listed_windows() -> Vec<WindowFacts> {
    let me = our_pid();
    let mut exes: Vec<(u32, String)> = vec![];
    top_level_windows()
        .into_iter()
        .map(window_facts)
        .filter(|w| is_listed_window(w, me))
        .map(|mut w| {
            w.exe = match exes.iter().find(|(p, _)| *p == w.pid) {
                Some((_, e)) => e.clone(),
                None => {
                    let e = process_exe(w.pid);
                    exes.push((w.pid, e.clone()));
                    e
                }
            };
            w
        })
        .collect()
}

/// This process's top-level windows (visible or not).
pub fn own_windows() -> Vec<HWND> {
    let me = our_pid();
    top_level_windows()
        .into_iter()
        .filter(|h| {
            let mut pid = 0u32;
            // SAFETY: read-only query.
            unsafe { GetWindowThreadProcessId(*h, Some(&mut pid)) };
            pid == me
        })
        .collect()
}

/// Displays, windows, apps, cameras and microphones.
pub fn list_devices() -> Devices {
    let mons = monitors();
    let facts: Vec<MonitorFacts> = mons.iter().map(|m| m.facts.clone()).collect();
    let displays = displays_from(&facts);
    let listed = listed_windows();
    let apps = apps_from_windows(
        listed
            .iter()
            .filter(|w| !w.exe.is_empty())
            .map(|w| (w.exe.as_str(), w.exe.as_str())),
    )
    .into_iter()
    .map(|mut a| {
        a.name = app_name(&a.bundle_id);
        a.bundle_id = app_id(&a.bundle_id);
        a
    })
    .collect();
    let windows = listed
        .iter()
        .map(|w| {
            let (i, frame) = window_frame(&w.rect, &facts);
            CaptureWindow {
                id: w.id,
                app: app_name(&w.exe),
                title: w.title.clone(),
                bundle_id: app_id(&w.exe),
                display_id: i as u32 + 1,
                frame,
            }
        })
        .collect();
    Devices {
        displays,
        windows,
        apps,
        cameras: super::camera::list_cameras(),
        mics: super::audio::list_mics(),
    }
}

/// Settings › Privacy switches for the camera and microphone (desktop apps). Screen
/// capture needs no grant on Windows.
pub fn permissions() -> Permissions {
    let base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
    let read = |cap: &str| {
        let key = format!(r"{base}\{cap}");
        consent_state(
            reg_string(HKEY_CURRENT_USER, &key, "Value").as_deref(),
            reg_string(HKEY_CURRENT_USER, &format!(r"{key}\NonPackaged"), "Value").as_deref(),
        )
    };
    Permissions {
        screen: PermissionState::Granted,
        microphone: read("microphone"),
        camera: read("webcam"),
    }
}

/// The Settings page for a permission.
pub fn privacy_uri(p: Permission) -> &'static str {
    match p {
        Permission::Camera => "ms-settings:privacy-webcam",
        Permission::Microphone => "ms-settings:privacy-microphone",
        // Nothing to grant; the closest page is the general privacy one.
        Permission::Screen => "ms-settings:privacy",
    }
}
