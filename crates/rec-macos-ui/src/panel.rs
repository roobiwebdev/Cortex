//! Floating-window behaviour for the recorder's chrome windows (HUD, toasts,
//! countdown, region picker, menu bar panel).
//!
//! Tauri (tao) creates these windows as `TaoWindow`, an NSWindow subclass
//! with Rust ivars and `canBecomeKeyWindow`/`canBecomeMainWindow` overrides.
//! We do NOT swap its class to NSPanel with `object_setClass`: NSPanel would
//! drop tao's overrides, and tao's `dealloc` (which drops its ivars) would no
//! longer run. Non-activation is applied instead with, in order of preference:
//! 1. the real `NSWindowStyleMaskNonactivatingPanel` bit if the window already
//!    is an NSPanel (e.g. one created natively);
//! 2. `-[NSWindow _setPreventsActivation:]` (private AppKit SPI, checked with
//!    `respondsToSelector:` first) for tao windows;
//! 3. otherwise nothing (reported as [`NonActivation::Unavailable`]); pair it
//!    with Tauri's `focusable(false)` for those windows.
//!
//! Capture exclusion has two layers. ScreenCaptureKit recordings made by the
//! engine already exclude this whole app via its `SCContentFilter`; the
//! `sharingType = none` set here additionally hides the window from other
//! apps' screenshots and legacy (CGWindowList) captures.

use std::ffi::c_void;

use objc2::runtime::{AnyObject, Bool};
use objc2::{ClassType, msg_send, sel};
use objc2_app_kit::{
    NSFloatingWindowLevel, NSPanel, NSPopUpMenuWindowLevel, NSStatusWindowLevel, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowLevel, NSWindowSharingType, NSWindowStyleMask,
};
use objc2_foundation::NSObjectProtocol;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::main_thread::run_on_main;

/**
 * SOURCE OF TRUTH KEYWORDS: FloatingKind, HUD, toast, countdown, region picker, menubar panel, window level
 * WHAT:  The recorder's floating chrome windows and the AppKit level each one uses.
 * WHY:   One place decides the stacking order: menu bar panel above
 *        everything (popUpMenu), countdown/toasts above the region overlay,
 *        region overlay over the menu bar and Dock, HUD just above normal windows.
 * WHERE: configure_floating; the Tauri shell when it creates these windows.
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatingKind {
    MenubarPanel,
    Hud,
    Toast,
    Countdown,
    RegionPicker,
}

impl FloatingKind {
    pub fn level(self) -> NSWindowLevel {
        match self {
            Self::MenubarPanel => NSPopUpMenuWindowLevel,
            Self::Hud => NSFloatingWindowLevel,
            Self::Toast | Self::Countdown => NSStatusWindowLevel + 1,
            Self::RegionPicker => NSStatusWindowLevel,
        }
    }

    /// The region picker is a full-screen translucent overlay; a shadow there
    /// only costs compositing. Everything else gets a native shadow.
    pub fn has_shadow(self) -> bool {
        !matches!(self, Self::RegionPicker)
    }

    pub fn collection_behavior(self) -> NSWindowCollectionBehavior {
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
    }
}

/// How (or whether) clicks on the window are kept from activating the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonActivation {
    /// The window is an NSPanel and has `NSWindowStyleMaskNonactivatingPanel`.
    Panel,
    /// `_setPreventsActivation:` was applied (tao NSWindow).
    PreventsActivation,
    /// Not available on this window/OS; use Tauri `focusable(false)`.
    Unavailable,
}

/**
 * SOURCE OF TRUTH KEYWORDS: configure_floating, NSPanel, non-activating, sharingType none, collectionBehavior, hidesOnDeactivate
 * WHAT:  Gives a Tauri-created window floating-panel behaviour: level per kind,
 *        visible on all Spaces and over full-screen apps, stays visible when the
 *        app deactivates, excluded from screen sharing, native shadow, and
 *        non-activating where AppKit allows it.
 * WHY:   The HUD and friends are web content but must behave like native panels
 *        and never appear in the user's recordings or screenshots.
 * WHERE: Tauri shell, right after creating each floating window (`ns_window()`).
 *
 * Runs synchronously on the main thread (hops there if needed). Returns `None`
 * for a null pointer.
 *
 * # Safety
 *
 * `ns_window` must be null or a valid `NSWindow*` alive for the duration of the call.
 */
pub unsafe fn configure_floating(
    ns_window: *mut c_void,
    kind: FloatingKind,
) -> Option<NonActivation> {
    if ns_window.is_null() {
        return None;
    }
    let addr = ns_window as usize;
    Some(run_on_main(move |_mtm| {
        // SAFETY: caller guarantees a live NSWindow; we are on the main thread.
        let window: &NSWindow = unsafe { &*(addr as *const NSWindow) };
        window.setLevel(kind.level());
        window.setCollectionBehavior(kind.collection_behavior());
        window.setHidesOnDeactivate(false);
        window.setSharingType(NSWindowSharingType::None);
        window.setHasShadow(kind.has_shadow());
        apply_non_activation(window)
    }))
}

fn apply_non_activation(window: &NSWindow) -> NonActivation {
    if window.isKindOfClass(NSPanel::class()) {
        window.setStyleMask(window.styleMask() | NSWindowStyleMask::NonactivatingPanel);
        return NonActivation::Panel;
    }
    let obj: &AnyObject = window.as_ref();
    let selector = sel!(_setPreventsActivation:);
    // SAFETY: respondsToSelector: is always safe; the private setter is only
    // sent after the runtime confirms it exists, with its documented BOOL argument.
    unsafe {
        let responds: bool = msg_send![obj, respondsToSelector: selector];
        if responds {
            let _: () = msg_send![obj, _setPreventsActivation: Bool::YES];
            return NonActivation::PreventsActivation;
        }
    }
    NonActivation::Unavailable
}

/**
 * SOURCE OF TRUTH KEYWORDS: set_ignores_mouse_events, click-through, passthrough, region overlay, countdown
 * WHAT:  Toggles click-through for a window (clicks reach the app underneath).
 * WHY:   The countdown and passive toasts must not swallow clicks meant for the
 *        app being recorded; the region picker needs clicks only while selecting.
 * WHERE: Tauri shell commands for countdown/toast/region windows.
 *
 * # Safety
 *
 * `ns_window` must be null or a valid `NSWindow*` alive for the duration of the call.
 */
pub unsafe fn set_ignores_mouse_events(ns_window: *mut c_void, ignore: bool) {
    if ns_window.is_null() {
        return;
    }
    let addr = ns_window as usize;
    run_on_main(move |_mtm| {
        // SAFETY: caller guarantees a live NSWindow; we are on the main thread.
        let window: &NSWindow = unsafe { &*(addr as *const NSWindow) };
        window.setIgnoresMouseEvents(ignore);
    });
}

/**
 * SOURCE OF TRUTH KEYWORDS: invalidate_shadow, native shadow, transparent window, content resize
 * WHAT:  Asks AppKit to recompute the window shadow from the current content.
 * WHY:   A transparent window's native shadow follows its painted pixels; after
 *        the web content changes size the old outline would stay.
 * WHERE: Tauri shell, after resizing a floating window to its content.
 *
 * # Safety
 *
 * `ns_window` must be null or a valid `NSWindow*` alive for the duration of the call.
 */
pub unsafe fn invalidate_shadow(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let addr = ns_window as usize;
    run_on_main(move |_mtm| {
        // SAFETY: caller guarantees a live NSWindow; we are on the main thread.
        let window: &NSWindow = unsafe { &*(addr as *const NSWindow) };
        window.invalidateShadow();
    });
}

/// The `NSWindow*` behind a window handle (an AppKit `NSView*`), on the main thread.
fn ns_window_of(handle: &impl HasWindowHandle) -> Option<*mut c_void> {
    let raw = handle.window_handle().ok()?.as_raw();
    let RawWindowHandle::AppKit(h) = raw else {
        return None;
    };
    let view = h.ns_view.as_ptr() as usize;
    let window = run_on_main(move |_mtm| {
        // SAFETY: the handle borrow (held by the caller for this call) keeps the
        // view alive; `window` is a plain getter and we are on the main thread.
        let view: &NSView = unsafe { &*(view as *const NSView) };
        view.window()
            .map(|w| objc2::rc::Retained::as_ptr(&w) as usize)
    })?;
    Some(window as *mut c_void)
}

/**
 * SOURCE OF TRUTH KEYWORDS: configure_floating_window, safe wrapper, HasWindowHandle, Tauri window, floating panel
 * WHAT:  Safe versions of `configure_floating`, `set_ignores_mouse_events` and
 *        `invalidate_shadow` for any window that exposes an AppKit window handle.
 * WHY:   The Tauri shell must not contain `unsafe`; borrowing the window handle
 *        is what keeps the NSWindow alive for the call.
 * WHERE: `apps/desktop/src-tauri/src/floating.rs`.
 */
pub fn configure_floating_window(
    window: &impl HasWindowHandle,
    kind: FloatingKind,
) -> Option<NonActivation> {
    let ns = ns_window_of(window)?;
    // SAFETY: `ns` comes from the live window the caller still borrows.
    unsafe { configure_floating(ns, kind) }
}

/// Safe [`set_ignores_mouse_events`] for a window handle.
pub fn set_window_ignores_mouse_events(window: &impl HasWindowHandle, ignore: bool) {
    if let Some(ns) = ns_window_of(window) {
        // SAFETY: `ns` comes from the live window the caller still borrows.
        unsafe { set_ignores_mouse_events(ns, ignore) }
    }
}

/// Lets screenshots see the window again (`sharingType = readOnly`). Debug evidence
/// only: the shipped windows stay excluded from capture.
pub fn set_window_capturable(window: &impl HasWindowHandle) {
    if let Some(ns) = ns_window_of(window) {
        let addr = ns as usize;
        run_on_main(move |_mtm| {
            // SAFETY: `addr` comes from the live window the caller still borrows; main thread.
            let window: &NSWindow = unsafe { &*(addr as *const NSWindow) };
            window.setSharingType(NSWindowSharingType::ReadOnly);
        });
    }
}

/// Safe [`invalidate_shadow`] for a window handle.
pub fn invalidate_window_shadow(window: &impl HasWindowHandle) {
    if let Some(ns) = ns_window_of(window) {
        // SAFETY: `ns` comes from the live window the caller still borrows.
        unsafe { invalidate_shadow(ns) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacking_order_matches_the_ui_study() {
        let l = |k: FloatingKind| k.level();
        assert!(l(FloatingKind::MenubarPanel) > l(FloatingKind::Countdown));
        assert_eq!(l(FloatingKind::Toast), l(FloatingKind::Countdown));
        assert!(l(FloatingKind::Countdown) > l(FloatingKind::RegionPicker));
        assert!(l(FloatingKind::RegionPicker) > l(FloatingKind::Hud));
        assert!(l(FloatingKind::Hud) > 0);
    }

    #[test]
    fn only_region_picker_skips_shadow() {
        assert!(!FloatingKind::RegionPicker.has_shadow());
        assert!(FloatingKind::Hud.has_shadow() && FloatingKind::Toast.has_shadow());
    }
}
