// Plain data only (no React, no Tauri): the tour tests import this file under Node.
import type { PlatformOs } from "./types";

export interface PlatformWords {
  /** The file manager by name. */
  fileManager: string;
  /** The reveal button: "Show in Finder" and friends. */
  revealLabel: string;
  /** The app that holds the privacy switches. */
  settingsApp: string;
  /** Where stream keys are kept, in a sentence. */
  keyStore: string;
  /** Where deleted recordings go. */
  trash: string;
  /** rec-core `DEFAULT_OUTPUT_DIR` on this platform. */
  defaultFolder: string;
  /** Where the status icon lives. */
  trayPlace: string;
  /** The Option / Alt modifier as a key cap. */
  altKey: string;
  /** The computer, in "sounds from this …". */
  computer: string;
  /** The oldest supported system, for About. */
  requirement: string;
}

/**
 * SOURCE OF TRUTH KEYWORDS: PLATFORM_WORDS, Finder, File Explorer, Files, System Settings, Keychain, Recycle Bin, menu bar, system tray
 * WHAT:  The words the UI uses for the operating system's own parts, per platform.
 * WHY:   Copy written for the Mac ("Reveal in Finder", "System Settings", "Keychain") reads
 *        wrong on Windows and Linux. Surfaces pick the word for `usePlatform()` from here.
 * WHERE: the tour's PLATFORM_FACTS, settings (storage, general, live, advanced), toasts, the
 *        menu bar panel, the HUD, Quick trim's export bar and the region picker.
 */
export const PLATFORM_WORDS: Record<PlatformOs, PlatformWords> = {
  macos: {
    fileManager: "Finder",
    revealLabel: "Show in Finder",
    settingsApp: "System Settings",
    keyStore: "the macOS Keychain",
    trash: "Trash",
    defaultFolder: "~/Movies/Cortex",
    trayPlace: "menu bar",
    altKey: "⌥",
    computer: "Mac",
    requirement: "macOS 15 or later",
  },
  windows: {
    fileManager: "File Explorer",
    revealLabel: "Show in File Explorer",
    settingsApp: "Settings",
    keyStore: "Windows Credential Manager",
    trash: "Recycle Bin",
    defaultFolder: "~/Videos/Cortex",
    trayPlace: "notification area",
    altKey: "Alt",
    computer: "PC",
    requirement: "Windows 10 (version 2004) or 11, 64-bit",
  },
  linux: {
    fileManager: "Files",
    revealLabel: "Show in Files",
    settingsApp: "Settings",
    keyStore: "your system keyring",
    trash: "Trash",
    defaultFolder: "~/Videos/Cortex",
    trayPlace: "system tray",
    altKey: "Alt",
    computer: "computer",
    requirement: "Linux with PipeWire",
  },
};
