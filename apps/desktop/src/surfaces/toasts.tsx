import * as React from "react";
import { Check, Copy, FolderOpen, Scissors, Settings, TriangleAlert, X } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../lib/engine";
import { useWindowed } from "../lib/floating";
import type { Toast, ToastKind } from "../lib/types";
import { MENUBAR_H } from "../prototype/desktop-backdrop";
import type { SurfaceProps } from "../prototype/surfaces";
import { ClipThumb } from "./floating/clip-thumb";
import { FloatingCard, PrototypeNote } from "./floating/floating-card";
import { useFlash } from "./floating/use-flash";

const TOAST_MS = 8000;
const EXIT_MS = 150;

const KIND: Record<ToastKind, { tag: string; tagClass: string; barClass: string }> = {
  saved: { tag: "Saved", tagClass: "bg-ok-tint text-ok", barClass: "bg-ok" },
  replay: {
    tag: "Replay saved",
    tagClass: "bg-accent-tint text-accent-text",
    barClass: "bg-accent-solid",
  },
  warning: {
    tag: "Warning",
    tagClass: "bg-warn-tint text-warn",
    barClass: "bg-warn",
  },
};

/**
 * SOURCE OF TRUTH KEYWORDS: ToastStack, toasts, saved toast, replay saved, disk warning, drain bar, UI-06, RPL-06
 * WHAT:  Up to three cards under the menu bar, newest on top, for "saved", "replay saved"
 *        and warnings, each with quick actions and an 8 s drain bar that pauses on hover.
 * WHY:   Rec only flashed a 4 s toolbar notice in the main window, which is usually hidden
 *        while recording. A capture-excluded card says what happened and offers the next step.
 * WHERE: Prototype surface "toasts"; the app's "toasts" window (non-activating, above full-screen apps).
 */
export function ToastStack({ navigate }: SurfaceProps) {
  const e = useEngine();
  const newestFirst = e.toasts.toReversed();
  const windowed = useWindowed();

  // In its own window: only the cards (the window sits under the menu bar and hides when empty).
  if (windowed)
    return newestFirst.length === 0 ? null : (
      <div className="flex w-[360px] flex-col gap-2" role="region" aria-label="Notifications" aria-live="polite">
        {newestFirst.map((t) => (
          <ToastCard key={t.id} toast={t} navigate={navigate} />
        ))}
      </div>
    );

  return (
    <div className="absolute inset-0">
      <div
        className="absolute right-3 flex w-[360px] flex-col gap-2"
        style={{ top: MENUBAR_H + 10 }}
        role="region"
        aria-label="Notifications"
        aria-live="polite"
      >
        {newestFirst.map((t) => (
          <ToastCard key={t.id} toast={t} navigate={navigate} />
        ))}
      </div>

      {newestFirst.length === 0 && (
        <div className="absolute inset-0 top-7 grid place-items-center p-6">
          <PrototypeNote
            title="Toasts appear top-right"
            actions={
              <>
                <Button variant="soft" size="sm" onClick={() => engine.demoToast("saved")}>
                  Recording saved
                </Button>
                <Button variant="soft" size="sm" onClick={() => engine.demoToast("replay")}>
                  Replay saved
                </Button>
                <Button variant="soft" size="sm" onClick={() => engine.demoToast("warning")}>
                  Low disk warning
                </Button>
              </>
            }
          >
            When a recording or replay is saved, a card slides in under the menu bar. It is left out of the recording,
            pauses while you hover it and clears itself after 8 seconds. Up to three stack, newest on top.
          </PrototypeNote>
        </div>
      )}
    </div>
  );
}

function ToastCard({ toast, navigate }: { toast: Toast; navigate: SurfaceProps["navigate"] }) {
  const e = useEngine();
  const kind = KIND[toast.kind];
  const recording = toast.recordingId ? e.recordings.find((r) => r.id === toast.recordingId) : undefined;
  const [left, setLeft] = React.useState(1);
  const [leaving, setLeaving] = React.useState(false);
  const [flashed, flash] = useFlash();
  const hovered = React.useRef(false);
  const setHover = (on: boolean) => {
    hovered.current = on;
  };

  const close = React.useCallback(() => {
    setLeaving(true);
    setTimeout(() => engine.dismissToast(toast.id), EXIT_MS);
  }, [toast.id]);

  // Drain the timer only while not hovered; dismiss when it runs out.
  React.useEffect(() => {
    let remaining = TOAST_MS;
    let last = performance.now();
    let frame = requestAnimationFrame(function tick(nowMs) {
      if (!hovered.current) remaining -= nowMs - last;
      last = nowMs;
      setLeft(Math.max(0, remaining / TOAST_MS));
      if (remaining <= 0) return close();
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  }, [close]);

  const path = recording?.path ?? `~/Movies/Cortex/replay-${toast.id}.mp4`;

  return (
    <FloatingCard
      role="status"
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
      onFocus={() => setHover(true)}
      onBlur={() => setHover(false)}
      className={cn(
        "relative overflow-hidden",
        leaving
          ? "duration-150 ease-in-quick animate-out fill-mode-forwards fade-out slide-out-to-right-8"
          : "duration-200 ease-out-quick animate-in fade-in slide-in-from-right-8",
      )}
    >
      <div className="flex gap-3 p-3 pr-9">
        {toast.kind === "warning" ? (
          <span aria-hidden className="grid size-10 shrink-0 place-items-center rounded-[10px] bg-warn-tint text-warn">
            <TriangleAlert className="size-5" />
          </span>
        ) : (
          <ClipThumb durationSecs={recording?.durationSecs} width={80} />
        )}
        <div className="min-w-0 flex-1">
          <span
            className={cn(
              "inline-flex h-[18px] items-center rounded-[5px] px-1.5 text-[10px] font-semibold tracking-[0.08em] uppercase",
              kind.tagClass,
            )}
          >
            {kind.tag}
          </span>
          <div className="mt-1 truncate text-[13px] font-semibold" title={toast.title}>
            {toast.title}
          </div>
          <div className="truncate text-xs text-muted-foreground tabular">{toast.detail}</div>

          <div className="mt-2 flex flex-wrap gap-1">
            {toast.kind === "warning" ? (
              <Button variant="soft" size="xs" onClick={() => navigate("settings")}>
                <Settings />
                Change folder
              </Button>
            ) : (
              <>
                <Button variant="soft" size="xs" onClick={() => flash("reveal")} title={`Show ${path} in Finder`}>
                  {flashed === "reveal" ? <Check /> : <FolderOpen />}
                  {flashed === "reveal" ? "Shown" : "Reveal"}
                </Button>
                <Button
                  variant="soft"
                  size="xs"
                  title="Copy the file to the clipboard"
                  onClick={() => {
                    void navigator.clipboard?.writeText(path).catch(() => undefined);
                    flash("copy");
                  }}
                >
                  {flashed === "copy" ? <Check /> : <Copy />}
                  {flashed === "copy" ? "Copied" : "Copy"}
                </Button>
                <Button variant="soft" size="xs" onClick={() => navigate("trim")}>
                  <Scissors />
                  Trim
                </Button>
              </>
            )}
          </div>
        </div>
      </div>

      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Dismiss"
        className="absolute top-2 right-2 rounded-full text-muted-foreground"
        onClick={close}
      >
        <X />
      </Button>

      <div className="absolute inset-x-0 bottom-0 h-0.5 bg-muted" aria-hidden>
        <div className={cn("h-full", kind.barClass)} style={{ width: `${left * 100}%` }} />
      </div>
    </FloatingCard>
  );
}
