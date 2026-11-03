import { browser } from "$app/env";

export type Platform = "mac" | "windows" | "linux" | "other";

/**
 * SOURCE OF TRUTH KEYWORDS: visitor platform, OS detection, download button, Windows, Linux, macOS
 * WHAT:  The visitor's operating system, read once in the browser from the user-agent hints.
 * WHY:   The page is prerendered, so the download buttons render for macOS first and switch to
 *        "Coming soon" after load for Windows and Linux visitors. Nothing is sent anywhere.
 * WHERE: routes/+page.svelte (hero, nav and install section).
 */
export function detectPlatform(): Platform {
  if (!browser) return "mac";
  const nav: Navigator & { userAgentData?: { platform?: string } } = navigator;
  const hint = `${nav.userAgentData?.platform ?? ""} ${nav.userAgent}`.toLowerCase();
  // iPhone and iPad user agents also mention "Mac OS X", so check them first.
  if (/iphone|ipad|ipod|android/.test(hint)) return "other";
  if (/mac/.test(hint)) return "mac";
  if (/win/.test(hint)) return "windows";
  if (/linux|cros|x11/.test(hint)) return "linux";
  return "other";
}
