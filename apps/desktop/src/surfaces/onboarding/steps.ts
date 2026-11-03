import type { Permission, PlatformOs } from "../../lib/types";

/** The tour, in order. The same steps on every platform; their copy differs. */
export const TOUR_STEPS = [
  "welcome",
  "permissions",
  "capture",
  "camera",
  "shortcut",
  "files",
  "trim",
  "replay-live",
  "done",
] as const;
export type TourStepId = (typeof TOUR_STEPS)[number];

export interface StepCopy {
  /** Short uppercase label above the headline, after "Step n of 9". */
  kicker: string;
  title: string;
  lede: string;
}

export type Need = "required" | "optional";

/** One permission row on the permissions step. */
export interface PermissionCopy {
  permission: Permission;
  need: Need;
  /** The permission's name as the system shows it. */
  name: string;
  /** What it unlocks, one line. */
  hint: string;
  allowLabel: string;
  /** What the user loses by skipping (shown on the last step). */
  skippedNote: string;
}

/**
 * How the system guards capture on each platform:
 * - "tcc": macOS privacy grants; Screen Recording is required and may need a relaunch.
 * - "privacy": Windows camera and microphone privacy switches; screen capture needs none.
 * - "portal": Linux desktop portals ask which screen or window to share each time.
 */
export type PermissionModel = "tcc" | "privacy" | "portal";

export interface PlatformFacts {
  permissionModel: PermissionModel;
  /** The permission rows the permissions step shows, in order. */
  permissions: readonly PermissionCopy[];
  /** Notes under the permission rows. */
  permissionNotes: readonly string[];
  /** Name of the app that holds privacy switches. */
  settingsApp: string;
  /** Key caps for the global shortcuts. */
  recordKeys: readonly string[];
  replayKeys: readonly string[];
  /** The keys as they read in a sentence, e.g. "⇧⌘R" or "Ctrl+Shift+R". */
  recordKeysText: string;
  replayKeysText: string;
  /** Spoken form for screen readers, e.g. "Shift Command R". */
  recordKeysLabel: string;
  replayKeysLabel: string;
  /** Where recordings go by default. */
  defaultFolder: string;
  /** "Show in Finder" and friends. */
  revealLabel: string;
  /** Where stream keys are kept. */
  keyStore: string;
  /** Where the tour can be reopened. */
  reopenHint: string;
  /** Welcome-step note about where the app lives. */
  homeNote: string;
}

/**
 * SOURCE OF TRUTH KEYWORDS: TOUR_COPY, PLATFORM_FACTS, onboarding copy, tour steps, per-platform, macos, windows, linux
 * WHAT:  Every word of the first-run tour, per platform, plus the platform facts its
 *        controls need (permission rows, shortcut keys, default folder, key store).
 * WHY:   The tour must read right on macOS now and on Windows and Linux later. Keeping
 *        the copy in one typed table means a new platform is a new entry, and the
 *        compiler lists every string it is missing.
 * WHERE: OnboardingWindow and its panels and stage scenes.
 */
export const PLATFORM_FACTS: Record<PlatformOs, PlatformFacts> = {
  macos: {
    permissionModel: "tcc",
    permissions: [
      {
        permission: "screen",
        need: "required",
        name: "Screen Recording",
        hint: "Displays, windows and apps, plus the sound your Mac plays (macOS lists it as Screen & System Audio Recording).",
        allowLabel: "Allow",
        skippedNote: "Required to record",
      },
      {
        permission: "camera",
        need: "optional",
        name: "Camera",
        hint: "The bubble of you in the corner.",
        allowLabel: "Allow",
        skippedNote: "No camera bubble",
      },
      {
        permission: "microphone",
        need: "optional",
        name: "Microphone",
        hint: "Your voice, on its own track.",
        allowLabel: "Allow",
        skippedNote: "Recordings have system audio only",
      },
    ],
    permissionNotes: [
      "After you allow screen recording, macOS may ask you to reopen Cortex. The tour picks up where you left off.",
      "Every few weeks macOS asks again to confirm. Choose Allow.",
    ],
    settingsApp: "System Settings",
    recordKeys: ["⇧", "⌘", "R"],
    replayKeys: ["⇧", "⌘", "S"],
    recordKeysText: "⇧⌘R",
    replayKeysText: "⇧⌘S",
    recordKeysLabel: "Shift Command R",
    replayKeysLabel: "Shift Command S",
    defaultFolder: "~/Movies/Cortex",
    revealLabel: "Show in Finder",
    keyStore: "the macOS Keychain",
    reopenHint: "Reopen this tour any time from Help › Show the Tour, or from Settings › General.",
    homeNote: "Cortex lives in your menu bar. Closing a window keeps it running.",
  },
  windows: {
    permissionModel: "privacy",
    permissions: [
      {
        permission: "camera",
        need: "optional",
        name: "Camera",
        hint: "The bubble of you in the corner.",
        allowLabel: "Open privacy settings",
        skippedNote: "No camera bubble",
      },
      {
        permission: "microphone",
        need: "optional",
        name: "Microphone",
        hint: "Your voice, on its own track.",
        allowLabel: "Open privacy settings",
        skippedNote: "Recordings have system audio only",
      },
    ],
    permissionNotes: [
      "Recording the screen needs no permission on Windows. Windows may draw a thin border around what is being recorded.",
      "Both switches live in Settings › Privacy & security. Turn on “Let desktop apps access” for each.",
    ],
    settingsApp: "Settings",
    recordKeys: ["Ctrl", "Shift", "R"],
    replayKeys: ["Ctrl", "Shift", "S"],
    recordKeysText: "Ctrl+Shift+R",
    replayKeysText: "Ctrl+Shift+S",
    recordKeysLabel: "Control Shift R",
    replayKeysLabel: "Control Shift S",
    defaultFolder: "Videos\\Cortex",
    revealLabel: "Show in Explorer",
    keyStore: "Windows Credential Manager",
    reopenHint: "Reopen this tour any time from Help › Show the tour, or from Settings › General.",
    homeNote: "Cortex lives in the notification area. Closing a window keeps it running.",
  },
  linux: {
    permissionModel: "portal",
    permissions: [
      {
        permission: "screen",
        need: "optional",
        name: "Screen sharing",
        hint: "Picked in a system dialog.",
        allowLabel: "Asked when you record",
        skippedNote: "Asked when you record",
      },
      {
        permission: "camera",
        need: "optional",
        name: "Camera",
        hint: "The bubble of you in the corner.",
        allowLabel: "Asked when needed",
        skippedNote: "No camera bubble",
      },
      {
        permission: "microphone",
        need: "optional",
        name: "Microphone",
        hint: "Your voice, on its own track.",
        allowLabel: "Asked when needed",
        skippedNote: "Recordings have system audio only",
      },
    ],
    permissionNotes: [
      "On Wayland this is the desktop portal: pick a screen or window, then Share.",
      "Flatpak and Snap installs may ask for the camera and mic on first use.",
    ],
    settingsApp: "Settings",
    recordKeys: ["Ctrl", "Shift", "R"],
    replayKeys: ["Ctrl", "Shift", "S"],
    recordKeysText: "Ctrl+Shift+R",
    replayKeysText: "Ctrl+Shift+S",
    recordKeysLabel: "Control Shift R",
    replayKeysLabel: "Control Shift S",
    defaultFolder: "~/Videos/Cortex",
    revealLabel: "Open folder",
    keyStore: "your system keyring",
    reopenHint: "Reopen this tour any time from Help › Show the tour, or from Settings › General.",
    homeNote: "Cortex lives in the system tray. Closing a window keeps it running.",
  },
};

const permissionsCopy: Record<PermissionModel, StepCopy> = {
  tcc: {
    kicker: "Permissions",
    title: "Let Cortex see and hear",
    lede: "macOS asks before any app records. Screen recording is the one Cortex needs; the camera and microphone are up to you.",
  },
  privacy: {
    kicker: "Privacy",
    title: "Camera and mic privacy settings",
    lede: "Windows lets you switch the camera and microphone on per app. Both are optional, and you can skip this step.",
  },
  portal: {
    kicker: "Sharing",
    title: "Your desktop asks what to share",
    lede: "When you start recording, the system asks which screen or window to share. Nothing to set up now.",
  },
};

function copyFor(p: PlatformOs): Record<TourStepId, StepCopy> {
  const f = PLATFORM_FACTS[p];
  return {
    welcome: {
      kicker: "Welcome to Cortex",
      title: "Record your screen, camera and voice",
      lede: "A quiet recorder that stays out of your way. This short tour shows the essentials and takes about two minutes.",
    },
    permissions: permissionsCopy[f.permissionModel],
    capture: {
      kicker: "What to record",
      title: "Pick what to record",
      lede: "A whole display, one window, an area you draw, or every window of one app. Cortex never records its own windows.",
    },
    camera: {
      kicker: "Camera",
      title: "Put yourself in the corner",
      lede: "Choose a shape and drag the bubble to any corner. Cortex can also keep the raw camera as its own track for editing later.",
    },
    shortcut: {
      kicker: "Shortcut",
      title: "Record from anywhere",
      lede: `Press ${f.recordKeysText} in any app to start, and again to stop. During a countdown it cancels.`,
    },
    files: {
      kicker: "Your files",
      title: "Where your files go",
      lede: "Recordings save straight to a folder on this computer. No account, nothing uploaded.",
    },
    trim: {
      kicker: "Quick trim",
      title: "Cut the ends in seconds",
      lede: "Drag the handles to trim the start and end. Fast mode cuts without re-encoding, so a long recording is done in a moment.",
    },
    "replay-live": {
      kicker: "Replay and live",
      title: "Catch what just happened, or go live",
      lede: `A replay buffer keeps the last minute in memory; ${f.replayKeysText} saves it. Go live streams to YouTube, Twitch, Kick or your own server.`,
    },
    done: {
      kicker: "All set",
      title: "You’re ready to record",
      lede: "That’s the tour. Start your first recording now, or close this window and start from the menu whenever you like.",
    },
  };
}

export const TOUR_COPY: Record<PlatformOs, Record<TourStepId, StepCopy>> = {
  macos: copyFor("macos"),
  windows: copyFor("windows"),
  linux: copyFor("linux"),
};

/**
 * Whether Continue must wait on this step: only macOS's Screen Recording grant gates the
 * tour, because nothing records without it.
 */
export function gatedPermission(p: PlatformOs, step: TourStepId): Permission | null {
  if (step !== "permissions") return null;
  const required = PLATFORM_FACTS[p].permissions.find((r) => r.need === "required");
  return PLATFORM_FACTS[p].permissionModel === "tcc" && required ? required.permission : null;
}

/** Where a resumed tour starts: the stored step if it is valid, else the first. */
export function resumeIndex(stored: string | null): number {
  const n = stored === null ? Number.NaN : Number(stored);
  return Number.isInteger(n) && n >= 0 && n < TOUR_STEPS.length ? n : 0;
}
