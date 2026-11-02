import * as React from "react";
import { LogicalSize, Window, getCurrentWindow } from "@tauri-apps/api/window";

import { useEngine } from "../lib/engine";
import { useStudioTheme } from "../lib/use-studio-theme";
import { SURFACES, type SurfaceId } from "../prototype/surfaces";

/** Logical width of the menu bar panel; must match `MENUBAR_WIDTH` in src-tauri/src/windows.rs. */
const MENUBAR_WIDTH = 340;

/**
 * SOURCE OF TRUTH KEYWORDS: TauriWindow, production router, window label, surface per window
 * WHAT:  Renders the one surface that belongs to the current Tauri window (by label),
 *        with no prototype chrome. Normal windows get a drag strip under the native
 *        traffic lights; floating panels get a transparent page.
 * WHY:   Every Tauri window loads the same index.html; the label decides what it shows.
 * WHERE: main.tsx when running inside Tauri. The browser keeps the PrototypeShell.
 */
export function TauriWindow({ label }: { label: string }) {
  const e = useEngine();
  useStudioTheme(e.settings.theme, e.settings.accentHue);
  const surface = SURFACES.find((s) => s.id === label);
  const floating = surface?.frame.kind === "overlay";

  React.useEffect(() => {
    if (floating) document.documentElement.dataset.floating = "";
  }, [floating]);

  const navigate = React.useCallback(
    (id: SurfaceId) => {
      void showWindow(id);
      // The menu bar panel gets out of the way once it has opened something else.
      if (label === "menubar") void getCurrentWindow().hide();
    },
    [label],
  );

  if (!surface) return null;
  const { Component } = surface;

  if (label === "menubar")
    return (
      <MenuBarHost>
        <Component cameraStream={null} navigate={navigate} />
      </MenuBarHost>
    );

  if (floating) {
    return (
      <div data-surface={label} className="relative h-full">
        <Component cameraStream={null} navigate={navigate} />
      </div>
    );
  }

  return (
    <div
      data-surface={label}
      className="relative h-full overflow-hidden bg-background text-foreground in-data-preview-hole:bg-transparent"
    >
      {/* The native title bar is transparent (Overlay style); this strip makes it draggable. */}
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-40 h-7" />
      <Component cameraStream={null} navigate={navigate} />
    </div>
  );
}

/** Sizes the menu bar window to its card, so the panel height follows its content. */
function MenuBarHost({ children }: { children: React.ReactNode }) {
  const ref = React.useRef<HTMLDivElement>(null);
  React.useEffect(() => {
    const card = ref.current?.firstElementChild;
    if (!card) return;
    const win = getCurrentWindow();
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(card.getBoundingClientRect().height);
      if (h > 0) void win.setSize(new LogicalSize(MENUBAR_WIDTH, h));
    });
    ro.observe(card);
    return () => ro.disconnect();
  }, []);
  return (
    <div ref={ref} data-surface="menubar">
      {children}
    </div>
  );
}

async function showWindow(label: SurfaceId) {
  const win = await Window.getByLabel(label);
  if (!win) return;
  await win.show();
  await win.setFocus();
}
