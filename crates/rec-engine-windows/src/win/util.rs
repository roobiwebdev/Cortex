//! SOURCE OF TRUTH KEYWORDS: com_init, mf_startup, qpc_secs, wide, windows_build, local_time, free_disk_bytes, Win32 helpers
//! WHAT:  Small Win32 helpers the Windows engine shares: per-thread COM, Media Foundation
//!        start-up, the QPC clock in seconds, UTF-16 strings, the OS build number, local
//!        time and free disk space.
//! WHY:   Every capture, audio and encoder thread needs the same set-up, and every time
//!        stamp (Windows.Graphics.Capture, WASAPI) is on the QPC clock in 100 ns units.
//! WHERE: Everything under `win/`.

use std::path::Path;
use std::sync::OnceLock;

use rec_core::library::LocalTime;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFStartup};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Registry::{HKEY, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::PCWSTR;

/// Join this thread to the multithreaded COM apartment (WinRT, Media Foundation and
/// WASAPI need it). Safe to call more than once; a thread already in another apartment
/// keeps it.
pub fn com_init() {
    // SAFETY: no reserved pointer; the result only says whether COM was already set up.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Start Media Foundation once per process. Returns its error message if it can't start.
pub fn mf_startup() -> Result<(), String> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            // SAFETY: plain call with constants; MFShutdown is never called, the process
            // keeps Media Foundation until it exits.
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }
                .map_err(|e| format!("Media Foundation could not start: {}", e.message()))
        })
        .clone()
}

fn qpc_frequency() -> i64 {
    static FREQ: OnceLock<i64> = OnceLock::new();
    *FREQ.get_or_init(|| {
        let mut f = 0i64;
        // SAFETY: `f` is a valid out-pointer.
        let _ = unsafe { QueryPerformanceFrequency(&mut f) };
        f.max(1)
    })
}

/// Now on the QPC clock, in seconds (the clock capture and audio time stamps use).
pub fn qpc_secs() -> f64 {
    let mut c = 0i64;
    // SAFETY: `c` is a valid out-pointer.
    let _ = unsafe { QueryPerformanceCounter(&mut c) };
    c as f64 / qpc_frequency() as f64
}

/// A QPC time stamp in 100 ns units (`SystemRelativeTime`, WASAPI's QPC position) in seconds.
pub fn hns_to_secs(hns: i64) -> f64 {
    hns as f64 / 1e7
}

/// NUL-terminated UTF-16 for Win32.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// UTF-16 up to the first NUL.
pub fn from_wide(w: &[u16]) -> String {
    let end = w.iter().position(|c| *c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

/// A `REG_SZ` value, or None.
pub fn reg_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (k, v) = (wide(key), wide(value));
    let mut buf = vec![0u16; 256];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: the key and value names are NUL-terminated; `buf` holds `bytes` bytes.
    let r = unsafe {
        RegGetValueW(
            root,
            PCWSTR(k.as_ptr()),
            PCWSTR(v.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    (r == ERROR_SUCCESS).then(|| from_wide(&buf))
}

/// The Windows build number (19045, 22631, ...), 0 when unknown.
pub fn windows_build() -> u32 {
    use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
    reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0)
}

pub fn local_time() -> LocalTime {
    // SAFETY: no arguments; returns a plain struct.
    let t = unsafe { GetLocalTime() };
    LocalTime {
        year: t.wYear,
        month: t.wMonth as u8,
        day: t.wDay as u8,
        hour: t.wHour as u8,
        minute: t.wMinute as u8,
        second: t.wSecond as u8,
    }
}

/// Free bytes for this user on the volume holding `dir` (or its nearest existing parent).
pub fn free_disk_bytes(dir: &Path) -> u64 {
    let Some(dir) = dir.ancestors().find(|d| d.exists()) else {
        return 0;
    };
    let w = wide(&dir.display().to_string());
    let mut free = 0u64;
    // SAFETY: `w` is NUL-terminated; `free` is a valid out-pointer.
    match unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), None, None) } {
        Ok(()) => free,
        Err(_) => 0,
    }
}

/// A Windows error as one line for the user.
pub fn msg(what: &str, e: &windows::core::Error) -> String {
    let m = e.message();
    let m = m.trim();
    if m.is_empty() {
        format!("{what} ({:#010x})", e.code().0)
    } else {
        format!("{what}: {m}")
    }
}
