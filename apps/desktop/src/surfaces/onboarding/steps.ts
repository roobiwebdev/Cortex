import type { Permission } from "../../lib/types";

export type StepId = "welcome" | "screen" | "microphone" | "camera" | "ready";

/** Order of the onboarding flow. */
export const STEPS: readonly StepId[] = ["welcome", "screen", "microphone", "camera", "ready"];

export type Need = "required" | "recommended" | "optional";

export interface PermissionStepCopy {
  permission: Permission;
  need: Need;
  /** Short name as macOS shows it in Privacy & Security. */
  name: string;
  title: string;
  lede: string;
  allowLabel: string;
  /** What the user loses by skipping (shown on the Ready summary). */
  skippedNote: string;
  /** Extra notes under the lede (OS quirks worth knowing up front). */
  notes: string[];
  /** Deep link into System Settings › Privacy & Security for this permission. */
  settingsUrl: string;
}

/**
 * SOURCE OF TRUTH KEYWORDS: PERMISSION_STEPS, onboarding copy, TCC, privacy panes, screen recording, microphone, camera
 * WHAT:  Copy and System Settings deep links for each permission step of onboarding.
 * WHY:   One table keeps the step screen, the Ready summary and (later) the main-window
 *        "missing permissions" banner saying the same thing about the same grant.
 * WHERE: OnboardingWindow and its PermissionPanel / ReadyPanel.
 */
export const PERMISSION_STEPS: Record<Permission, PermissionStepCopy> = {
  screen: {
    permission: "screen",
    need: "required",
    name: "Screen & System Audio Recording",
    title: "Let Cortex see your screen",
    lede: "Cortex needs Screen Recording to capture your displays, windows and apps. The same permission covers system audio, so what you hear gets recorded too.",
    allowLabel: "Allow screen recording",
    skippedNote: "Required to record",
    notes: [
      "macOS may ask you to quit and reopen Cortex after you allow it. You will come back to this step.",
      "Every few weeks macOS asks again to confirm. That is normal; choose Allow.",
    ],
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
  },
  microphone: {
    permission: "microphone",
    need: "recommended",
    name: "Microphone",
    title: "Add your voice",
    lede: "Allow the microphone to narrate as you record. Your voice is kept on its own track, so you can fix its level later.",
    allowLabel: "Allow microphone",
    skippedNote: "Recordings have system audio only",
    notes: [],
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
  },
  camera: {
    permission: "camera",
    need: "optional",
    name: "Camera",
    title: "Put yourself in the frame",
    lede: "Allow the camera to add a bubble of yourself to recordings. You choose its shape and corner, and it can also be saved as a separate track.",
    allowLabel: "Allow camera",
    skippedNote: "No camera bubble",
    notes: [],
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera",
  },
};

export const NEED_LABEL: Record<Need, string> = {
  required: "Required",
  recommended: "Recommended",
  optional: "Optional",
};
