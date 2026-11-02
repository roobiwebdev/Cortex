import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { commands } from "../ipc/bindings";

/** Viewport rect in CSS px. */
export type HoleRect = { x: number; y: number; w: number; h: number };

/** True when the page runs in the Tauri main window, where the native preview lives. */
export function hasNativePreview(): boolean {
  return isTauri() && getCurrentWindow().label === "main";
}

/**
 * SOURCE OF TRUTH KEYWORDS: useNativePreviewHole, native preview, transparent hole, setPreviewRect, setPreviewEnabled, data-preview-hole
 * WHAT:  Turns the native live preview on while `enabled`, keeps it placed over the `hole`
 *        element (resize, layout shifts, display scale changes), and marks the page with
 *        `html[data-preview-hole]` so the window's ancestors stop painting backgrounds.
 *        Returns the `stage` element's viewport rect, for painting the page around it.
 * WHY:   Video frames never enter the webview: a native view sits under the transparent
 *        page (src-tauri/src/preview.rs). Only the main window may call these commands.
 * WHERE: PreviewStage in the Tauri main window.
 */
export function useNativePreviewHole(
  enabled: boolean,
  hole: React.RefObject<HTMLElement | null>,
  stage: React.RefObject<HTMLElement | null>,
): HoleRect | null {
  const [stageRect, setStageRect] = React.useState<HoleRect | null>(null);

  React.useLayoutEffect(() => {
    if (!enabled) return;
    const root = document.documentElement;
    root.dataset.previewHole = "";
    let last = "";
    let frame = 0;

    const measure = () => {
      frame = 0;
      const h = hole.current;
      const s = stage.current;
      if (!h || !s) return;
      const r = h.getBoundingClientRect();
      const radius = Number.parseFloat(getComputedStyle(h).borderTopLeftRadius) || 0;
      const key = [r.left, r.top, r.width, r.height, radius].join(",");
      if (key !== last) {
        last = key;
        void commands.setPreviewRect(r.left, r.top, r.width, r.height, radius);
      }
      const sr = s.getBoundingClientRect();
      setStageRect((prev) =>
        prev && prev.x === sr.left && prev.y === sr.top && prev.w === sr.width && prev.h === sr.height
          ? prev
          : { x: sr.left, y: sr.top, w: sr.width, h: sr.height },
      );
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };

    const ro = new ResizeObserver(schedule);
    if (hole.current) ro.observe(hole.current);
    if (stage.current) ro.observe(stage.current);
    ro.observe(document.body);
    window.addEventListener("resize", schedule);
    let unlistenScale: (() => void) | undefined;
    let disposed = false;
    void getCurrentWindow()
      .onScaleChanged(() => {
        last = "";
        schedule();
      })
      .then((u) => {
        if (disposed) u();
        else unlistenScale = u;
      });

    measure();
    void commands.setPreviewEnabled(true);

    return () => {
      disposed = true;
      if (frame) cancelAnimationFrame(frame);
      ro.disconnect();
      window.removeEventListener("resize", schedule);
      unlistenScale?.();
      delete root.dataset.previewHole;
      void commands.setPreviewEnabled(false);
    };
  }, [enabled, hole, stage]);

  return enabled ? stageRect : null;
}
