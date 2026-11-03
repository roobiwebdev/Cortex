import { useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";

import { commands } from "../ipc/bindings";
import type { PlatformOs } from "./types";

const inTauri = isTauri();
export const PLATFORMS: readonly PlatformOs[] = ["macos", "windows", "linux"];

function isPlatform(v: string | null): v is PlatformOs {
  return v === "macos" || v === "windows" || v === "linux";
}

/** The harness starts on `?platform=` when given (screenshots), else macOS. */
function harnessStart(): PlatformOs {
  if (typeof location === "undefined") return "macos";
  const p = new URLSearchParams(location.search).get("platform");
  return isPlatform(p) ? p : "macos";
}

let current: PlatformOs = inTauri ? "macos" : harnessStart();
const listeners = new Set<() => void>();

/**
 * SOURCE OF TRUTH KEYWORDS: loadPlatform, usePlatform, setHarnessPlatform, platform, macos, windows, linux
 * WHAT:  The platform the app runs on. Inside Tauri it comes from Rust (`platform` command,
 *        `rec_core::platform::PlatformOs::current()`), read once before the first render; the browser
 *        harness can switch it to review the Windows and Linux screens.
 * WHY:   Copy and steps that differ per platform (the tour's permission step, shortcut keys,
 *        folder names) read one value instead of sniffing the user agent.
 * WHERE: main.tsx (`loadPlatform`), the onboarding tour, the prototype shell's platform switch.
 */
export async function loadPlatform(): Promise<void> {
  if (!inTauri) return;
  current = await commands.platform().catch(() => current);
  for (const l of listeners) l();
}

/** Harness only: show the screens of another platform. Ignored inside Tauri. */
export function setHarnessPlatform(p: PlatformOs) {
  if (inTauri || p === current) return;
  current = p;
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function usePlatform(): PlatformOs {
  return useSyncExternalStore(subscribe, () => current);
}
