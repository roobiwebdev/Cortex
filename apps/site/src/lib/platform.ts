import { browser } from "$app/env";

import type { LinuxArch } from "./site.ts";

export type Platform = "mac" | "windows" | "linux" | "other";

interface UAData {
  platform?: string;
  getHighEntropyValues?: (hints: string[]) => Promise<{ architecture?: string; bitness?: string }>;
}

function uaData(): UAData | undefined {
  const nav: Navigator & { userAgentData?: UAData } = navigator;
  return nav.userAgentData;
}

/**
 * SOURCE OF TRUTH KEYWORDS: visitor platform, OS detection, detectArch, download button, Windows, Linux, macOS
 * WHAT:  The visitor's operating system and, on Linux, the CPU architecture, read in the browser
 *        from the user-agent hints.
 * WHY:   The page is prerendered with the macOS download; after load the hero and the install
 *        tabs switch to the visitor's own installer. Nothing is sent anywhere.
 * WHERE: routes/+page.svelte (nav, hero and install section).
 */
export function detectPlatform(): Platform {
  if (!browser) return "mac";
  const hint = `${uaData()?.platform ?? ""} ${navigator.userAgent}`.toLowerCase();
  // iPhone and iPad user agents also mention "Mac OS X", so check them first; Android says Linux.
  if (/iphone|ipad|ipod|android/.test(hint)) return "other";
  if (/mac/.test(hint)) return "mac";
  if (/win/.test(hint)) return "windows";
  if (/linux|cros|x11/.test(hint)) return "linux";
  return "other";
}

/**
 * The Linux build that fits this CPU. Chromium answers through the high-entropy hints; Firefox
 * puts `aarch64` in its user agent. Anything unknown gets x86_64, the common case.
 */
export async function detectLinuxArch(): Promise<LinuxArch> {
  if (!browser) return "x86_64";
  if (/aarch64|arm64|armv8/i.test(navigator.userAgent)) return "aarch64";
  try {
    const values = await uaData()?.getHighEntropyValues?.(["architecture", "bitness"]);
    if (values?.architecture === "arm") return "aarch64";
  } catch {
    // The browser refused the hint; keep the default.
  }
  return "x86_64";
}
