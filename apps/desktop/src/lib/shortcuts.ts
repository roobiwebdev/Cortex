// Plain functions only (no React, no Tauri): the tour tests import this file under Node.
import type { PlatformOs } from "./types";

/** The app's fixed shortcuts. Record and save-replay are global; the rest work in Cortex. */
export type ShortcutAction = "record" | "replay" | "settings" | "quit" | "cancel";

interface Combo {
  /** ⌘ on macOS, Ctrl on Windows and Linux. */
  primary: boolean;
  shift: boolean;
  key: string;
}

// What the shell registers: macOS ⇧⌘R / ⇧⌘S (Carbon), Windows Ctrl+Shift+R / S and Esc
// (`RegisterHotKey`), Linux CTRL+SHIFT+r / s (GlobalShortcuts portal; Esc in-app only).
const COMBOS: Record<ShortcutAction, Combo> = {
  record: { primary: true, shift: true, key: "R" },
  replay: { primary: true, shift: true, key: "S" },
  settings: { primary: true, shift: false, key: "," },
  quit: { primary: true, shift: false, key: "Q" },
  cancel: { primary: false, shift: false, key: "Esc" },
};

/**
 * SOURCE OF TRUTH KEYWORDS: shortcutKeys, formatShortcut, shortcutSpoken, useShortcut, isPrimaryModifier, hotkey label, ⇧⌘R, Ctrl+Shift+R
 * WHAT:  The key caps, the written form ("⇧⌘R" / "Ctrl+Shift+R") and the spoken form of each
 *        fixed shortcut on a platform, and the primary-modifier test for key handlers.
 * WHY:   The labels were hard-coded as ⇧⌘R in every surface, which is wrong on Windows and
 *        Linux. One table means each label matches the keys the shell really registers.
 * WHERE: ShortcutKbd and useShortcut (components/shortcut-kbd.tsx), the tour's PLATFORM_FACTS,
 *        and the key handlers of the main window and the harness.
 */
export function shortcutKeys(action: ShortcutAction, platform: PlatformOs): readonly string[] {
  const c = COMBOS[action];
  // Apple's order is ⇧⌘ then the key; Windows and Linux write Ctrl+Shift+key.
  if (platform === "macos") return [...(c.shift ? ["⇧"] : []), ...(c.primary ? ["⌘"] : []), c.key];
  return [...(c.primary ? ["Ctrl"] : []), ...(c.shift ? ["Shift"] : []), c.key];
}

export function formatShortcut(action: ShortcutAction, platform: PlatformOs): string {
  return shortcutKeys(action, platform).join(platform === "macos" ? "" : "+");
}

const SPOKEN: Record<string, string> = { "⇧": "Shift", "⌘": "Command", Ctrl: "Control", ",": "Comma", Esc: "Escape" };

/** For screen readers, e.g. "Shift Command R" or "Control Shift R". */
export function shortcutSpoken(action: ShortcutAction, platform: PlatformOs): string {
  return shortcutKeys(action, platform)
    .map((k) => SPOKEN[k] ?? k)
    .join(" ");
}

/** ⌘ on macOS, Ctrl elsewhere: the modifier of the app's own shortcuts. */
export function isPrimaryModifier(ev: { metaKey: boolean; ctrlKey: boolean }, platform: PlatformOs): boolean {
  return platform === "macos" ? ev.metaKey : ev.ctrlKey;
}
