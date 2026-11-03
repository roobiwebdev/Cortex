import * as React from "react";

import { commands, type AppWindow } from "../ipc/bindings";
import { useClock, useEngine, useEngineCalm } from "../lib/engine";
import type { EngineState } from "../lib/types";
import { WindowedContext, floatingWindow } from "../lib/floating";
import { useStudioTheme } from "../lib/use-studio-theme";
import { SURFACES, type SurfaceId } from "../prototype/surfaces";

/** Floating windows whose size follows their content (the region picker fills a display). */
const FITTED: ReadonlySet<string> = new Set(["menubar", "hud", "toasts", "countdown"]);
/** Floating windows that close when they open something else or press Esc. */
const TRANSIENT: ReadonlySet<string> = new Set(["menubar", "region"]);

const themeOf = (s: EngineState) => s.settings.theme;
const accentHueOf = (s: EngineState) => s.settings.accentHue;

/**
 * SOURCE OF TRUTH KEYWORDS: TauriWindow, production router, window label, surface per window
 * WHAT:  Renders the one surface that belongs to the current Tauri window (by label),
 *        with no prototype chrome. Normal windows get a drag strip under the native
 *        traffic lights; floating panels get a transparent page sized to their card.
 * WHY:   Every Tauri window loads the same index.html; the label decides what it shows.
 *        Floating windows are placed and shown by Rust (`floating.rs`); the page only
 *        reports its size and asks to open or hide.
 * WHERE: main.tsx when running inside Tauri. The browser keeps the PrototypeShell.
 */
export function TauriWindow({ label }: { label: string }) {
  // Only the theme: the root must not re-render (and re-render the surface) on engine updates.
  useStudioTheme(useEngine(themeOf), useEngine(accentHueOf));
  const surface = SURFACES.find((s) => s.id === label);
  const floating = surface?.frame.kind === "overlay";

  React.useEffect(() => {
    if (floating) document.documentElement.dataset.floating = label;
  }, [floating, label]);

  const navigate = React.useCallback(
    (id: SurfaceId) => {
      if (id === "menubar" || id === "region" || id === "hud") floatingWindow.open(id);
      else if (id === "main" || id === "settings" || id === "onboarding" || id === "trim") openWindow(id);
      // A transient panel gets out of the way once it has opened something else.
      if (TRANSIENT.has(label)) floatingWindow.hide();
    },
    [label],
  );

  // Tell the shell this window is listening (it holds toasts back until then).
  React.useEffect(() => {
    commands.windowReady().catch(report("window_ready"));
  }, []);

  // Esc closes the menu bar panel (the region picker handles its own Esc).
  React.useEffect(() => {
    if (label !== "menubar") return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape" && !ev.defaultPrevented) floatingWindow.hide();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [label]);

  if (!surface) return null;
  const { Component } = surface;

  if (floating) {
    const content = <Component cameraStream={null} navigate={navigate} />;
    return (
      <WindowedContext.Provider value>
        {FITTED.has(label) ? (
          <FittedHost label={label}>{content}</FittedHost>
        ) : (
          <div data-surface={label} className="relative h-full">
            {content}
          </div>
        )}
      </WindowedContext.Provider>
    );
  }

  return (
    <div
      data-surface={label}
      className="relative h-full overflow-hidden bg-background text-foreground in-data-preview-hole:bg-transparent"
    >
      {/* The native title bar is transparent (Overlay style); this strip makes it draggable. */}
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-10 h-7" />
      <Component cameraStream={null} navigate={navigate} />
    </div>
  );
}

/**
 * Sizes the window to its content. Measured after every engine update as well as on
 * resize, because a hidden window may not deliver ResizeObserver callbacks (the toasts
 * window must learn it has a toast while still hidden).
 */
function FittedHost({ label, children }: { label: string; children: React.ReactNode }) {
  // Real changes, plus the clock once a second (its text can widen the HUD).
  const e = useEngineCalm();
  const secs = useClock();
  const ref = React.useRef<HTMLDivElement>(null);
  const sent = React.useRef("");

  const measure = React.useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const w = Math.ceil(r.width);
    const h = Math.ceil(r.height);
    const key = `${w}x${h}`;
    if (key === sent.current) return;
    sent.current = key;
    floatingWindow.fit(w, h);
  }, []);

  React.useLayoutEffect(measure, [measure, e, secs]);
  React.useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [measure]);

  return (
    <div ref={ref} data-surface={label} className="w-fit">
      {children}
    </div>
  );
}

/** Windows are created on first use, so Rust opens them (`open_window`), not `Window.getByLabel`. */
function openWindow(window: AppWindow) {
  commands.openWindow(window).catch(report("open_window"));
}

function report(what: string) {
  return (err: unknown) => console.error(`cortex: ${what} failed`, err);
}
