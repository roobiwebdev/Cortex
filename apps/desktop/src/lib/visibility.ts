/**
 * SOURCE OF TRUTH KEYWORDS: visibility, data-hidden, document.hidden, visibilitychange, pause animations, hidden window
 * WHAT:  Mirrors `document.hidden` onto `html[data-hidden]` (which pauses all CSS animations,
 *        see styles.css) and lets other modules defer work until the page is visible again.
 * WHY:   Every Tauri window is its own WebKit process. A hidden window that keeps animating
 *        or re-rendering on state events costs CPU for nothing (about 3% per window measured
 *        in the phase 2 gate). WebKit marks ordered-out and occluded windows hidden.
 * WHERE: main.tsx calls `trackVisibility()` once; `engine.ts` defers state renders.
 */

const shownListeners = new Set<() => void>();

export function pageHidden(): boolean {
  return typeof document !== "undefined" && document.hidden;
}

/** Runs `l` each time the page becomes visible. Returns the unsubscribe function. */
export function onPageShown(l: () => void): () => void {
  shownListeners.add(l);
  return () => {
    shownListeners.delete(l);
  };
}

function syncVisibility() {
  const hidden = document.hidden;
  document.documentElement.toggleAttribute("data-hidden", hidden);
  if (!hidden) for (const l of shownListeners) l();
}

export function trackVisibility() {
  document.addEventListener("visibilitychange", syncVisibility);
  syncVisibility();
}
