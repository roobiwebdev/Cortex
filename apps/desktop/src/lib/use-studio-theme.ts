import * as React from "react";

import type { ThemePref } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: useStudioTheme, theme, dark mode, accent hue, --accent-h, studio class
 * WHAT:  Applies the studio theme to <html>: `studio` class, `dark` from the user's
 *        preference (System follows macOS), and the accent hue variable.
 * WHY:   One place decides light/dark and accent so every window matches.
 * WHERE: the app root (prototype shell now, each Tauri window later).
 */
export function useStudioTheme(pref: ThemePref, accentHue: number) {
  const [systemDark, setSystemDark] = React.useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  React.useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const on = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);

  const dark = pref === "dark" || (pref === "system" && systemDark);
  React.useEffect(() => {
    const el = document.documentElement;
    el.classList.add("studio");
    el.classList.toggle("dark", dark);
    el.style.setProperty("--accent-h", String(accentHue));
  }, [dark, accentHue]);
  return dark;
}

/** Accent presets. Red hues are deliberately excluded: red means "recording". */
export const ACCENT_PRESETS = [
  { hue: 188, label: "Lagoon" },
  { hue: 222, label: "Cobalt" },
  { hue: 262, label: "Iris" },
  { hue: 152, label: "Fern" },
  { hue: 38, label: "Amber" },
  { hue: 210, label: "Slate" },
] as const;
