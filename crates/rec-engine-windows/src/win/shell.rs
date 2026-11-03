//! SOURCE OF TRUTH KEYWORDS: exclude_from_capture, WDA_EXCLUDEFROMCAPTURE, floating panel, WS_EX_NOACTIVATE, RegisterHotKey, message-only window, WM_HOTKEY, ShellExecuteW, console ctrl handler
//! WHAT:  The native window and process behaviour the Tauri shell needs on Windows: keep a
//!        window out of every screen capture, make a window a non-activating always-on-top
//!        panel, global hotkeys, opening `ms-settings:` pages, and Ctrl+C / console close.
//! WHY:   The shell has no `unsafe` (the desktop rules keep it in the engine crates), so its
//!        `platform/windows.rs` calls these with a window's raw handle, as it calls
//!        `rec_macos_ui` on macOS. Hotkeys use a message-only window created on the calling
//!        (main) thread, so `WM_HOTKEY` arrives through the app's own message loop and the
//!        callback runs on the main thread, as the `Hotkeys` trait promises.
//! WHERE: `apps/desktop/src-tauri/src/platform/windows.rs`; `win::session` (own windows).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GWL_EXSTYLE, GetWindowLongW, HWND_MESSAGE, HWND_TOPMOST,
    RegisterClassExW, SW_SHOWNORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetWindowDisplayAffinity, SetWindowLongW, SetWindowPos, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_HOTKEY, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR};

use super::util::{msg, wide};

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// Keep a window out of every screen capture and screenshot (Windows 10 2004+). Must be
/// called from this process; works from any of its threads.
pub fn exclude_from_capture(raw: isize) -> Result<(), String> {
    // SAFETY: the handle is one of this process's windows (from Tauri or EnumWindows).
    unsafe { SetWindowDisplayAffinity(hwnd(raw), WDA_EXCLUDEFROMCAPTURE) }
        .map_err(|e| msg("Could not hide the window from captures", &e))
}

/// Let captures see a window again (debug runs that screenshot the floating windows).
pub fn allow_capture(raw: isize) {
    // SAFETY: as above.
    let _ = unsafe { SetWindowDisplayAffinity(hwnd(raw), WDA_NONE) };
}

/// Hide every top-level window of this process from captures. Returns how many failed.
pub fn exclude_own_windows() -> usize {
    super::devices::own_windows()
        .into_iter()
        .filter(|h| exclude_from_capture(h.0 as isize).is_err())
        .count()
}

/// A panel with no taskbar button that stays above other windows. `no_activate`: it never
/// takes focus either (HUD, toasts, countdown); panels that take typing (the region
/// picker's Enter and Esc, the menu bar panel) leave it off.
pub fn make_floating(raw: isize, no_activate: bool) -> Result<(), String> {
    let h = hwnd(raw);
    // SAFETY: style reads and writes on one of this process's windows.
    unsafe {
        let ex = WINDOW_EX_STYLE(GetWindowLongW(h, GWL_EXSTYLE) as u32);
        let mut want = ex | WS_EX_TOOLWINDOW;
        if no_activate {
            want |= WS_EX_NOACTIVATE;
        }
        if want != ex {
            SetWindowLongW(h, GWL_EXSTYLE, want.0 as i32);
        }
        SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
        .map_err(|e| msg("Could not float the window", &e))
    }
}

/// Open a URI with its handler (`ms-settings:privacy-webcam`, a folder path, ...).
pub fn open_uri(uri: &str) -> Result<(), String> {
    let (verb, target) = (wide("open"), wide(uri));
    // SAFETY: NUL-terminated strings that outlive the call.
    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("Windows could not open {uri}"))
    }
}

// ---- Global hotkeys -------------------------------------------------------------------

/// The virtual-key codes the shell uses.
pub const VK_R: u32 = 0x52;
pub const VK_S: u32 = 0x53;
pub const VK_ESCAPE: u32 = 0x1B;

thread_local! {
    static HOST: RefCell<Option<isize>> = const { RefCell::new(None) };
    static CALLBACKS: RefCell<HashMap<i32, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
}

unsafe extern "system" fn hotkey_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if m == WM_HOTKEY {
        let id = w.0 as i32;
        // Clone out of the map first: the callback may register or unregister hotkeys.
        let f = CALLBACKS.with(|c| c.borrow().get(&id).cloned());
        if let Some(f) = f {
            f();
        }
        return LRESULT(0);
    }
    // SAFETY: default handling for every other message of our own window.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// This thread's message-only window for `WM_HOTKEY`, made on first use.
fn host() -> Result<HWND, String> {
    if let Some(h) = HOST.with(|h| *h.borrow()) {
        return Ok(hwnd(h));
    }
    let class = wide("CortexHotkeys");
    // SAFETY: the class and window are this module's own; strings outlive the calls.
    let h = unsafe {
        let instance = GetModuleHandleW(PCWSTR::null()).map_err(|e| msg("Hotkeys", &e))?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(hotkey_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        // A second registration (another thread) fails harmlessly; the class exists.
        RegisterClassExW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| msg("Hotkeys", &e))?
    };
    HOST.with(|slot| *slot.borrow_mut() = Some(h.0 as isize));
    Ok(h)
}

/// Register a system-wide hotkey `id` (replacing an earlier one with the same id): `vk`
/// with Ctrl+Shift when `ctrl_shift`, else alone. `f` runs on this thread, which must run
/// a message loop (Tauri's main thread). The error says why (usually another app has it).
pub fn register_hotkey(id: u32, vk: u32, ctrl_shift: bool, f: Box<dyn Fn()>) -> Result<(), String> {
    let h = host()?;
    let id = i32::try_from(id).map_err(|_| "Hotkey id out of range".to_string())?;
    let mods = if ctrl_shift {
        HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_NOREPEAT.0)
    } else {
        MOD_NOREPEAT
    };
    // SAFETY: our own message-only window on this thread.
    unsafe {
        let _ = UnregisterHotKey(Some(h), id);
        RegisterHotKey(Some(h), id, mods, vk).map_err(|e| msg("The shortcut is taken", &e))?;
    }
    CALLBACKS.with(|c| c.borrow_mut().insert(id, Rc::from(f)));
    Ok(())
}

pub fn unregister_hotkey(id: u32) {
    let Ok(id) = i32::try_from(id) else { return };
    if let Some(h) = HOST.with(|h| *h.borrow()) {
        // SAFETY: our own window; failing for an unknown id is harmless.
        let _ = unsafe { UnregisterHotKey(Some(hwnd(h)), id) };
    }
    CALLBACKS.with(|c| c.borrow_mut().remove(&id));
}

// ---- Termination ----------------------------------------------------------------------

type Terminate = Box<dyn Fn() + Send>;
static ON_TERMINATE: Mutex<Option<Terminate>> = Mutex::new(None);

unsafe extern "system" fn console_ctrl(_kind: u32) -> BOOL {
    if let Ok(g) = ON_TERMINATE.lock()
        && let Some(f) = g.as_ref()
    {
        f();
        return true.into();
    }
    false.into()
}

/// Call `f` on Ctrl+C, Ctrl+Break or console close (runs with `tauri dev` or from a
/// terminal; a GUI launch has no console and gets no signal).
pub fn on_terminate(f: Terminate) -> std::io::Result<()> {
    if let Ok(mut g) = ON_TERMINATE.lock() {
        *g = Some(f);
    }
    // SAFETY: registers a static handler function.
    unsafe { SetConsoleCtrlHandler(Some(console_ctrl), true) }.map_err(std::io::Error::other)
}
