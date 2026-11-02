import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { commands, type FloatingSurface } from "../ipc/bindings";

const inTauri = isTauri();

/**
 * SOURCE OF TRUTH KEYWORDS: WindowedContext, useWindowed, floating surface in its own window, harness vs app
 * WHAT:  True when a floating surface (HUD, toasts, countdown, menu bar panel) renders in its
 *        own Tauri window; false in the browser harness, where it sits over the fake desktop.
 * WHY:   In a real window the surface draws only its card, sized to content, with no fake
 *        desktop positioning or prototype notes. The harness keeps its review layout.
 * WHERE: `app/tauri-window.tsx` provides it; the floating surfaces read it.
 */
export const WindowedContext = React.createContext(false);

export function useWindowed(): boolean {
  return React.useContext(WindowedContext);
}

function report(what: string) {
  return (err: unknown) => console.error(`cortex: ${what} failed`, err);
}

/**
 * SOURCE OF TRUTH KEYWORDS: floatingWindow, fitFloating, openFloating, hideFloating, startDragging
 * WHAT:  The floating-window commands a surface may call. No-ops outside Tauri.
 * WHY:   Placement and visibility are Rust's (it knows the displays and engine state);
 *        surfaces only report their size or ask to open or hide.
 * WHERE: `app/tauri-window.tsx`, HudSurface (hide, drag).
 */
export const floatingWindow = {
  fit(width: number, height: number) {
    if (inTauri) commands.fitFloating(width, height).catch(report("fit_floating"));
  },
  open(surface: FloatingSurface) {
    if (inTauri) commands.openFloating(surface).catch(report("open_floating"));
  },
  hide() {
    if (inTauri) commands.hideFloating().catch(report("hide_floating"));
  },
  startDragging() {
    if (inTauri) getCurrentWindow().startDragging().catch(report("start_dragging"));
  },
};
