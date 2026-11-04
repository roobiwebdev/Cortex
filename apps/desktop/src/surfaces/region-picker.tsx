import * as React from "react";
import { AppWindow } from "lucide-react";

import { isTauri } from "@tauri-apps/api/core";

import { CoachMark } from "../components/coach-mark";
import { engine, useEngine } from "../lib/engine";
import { usePlatformWords } from "../lib/platform";
import type { CaptureTarget, Display } from "../lib/types";
import { onPageShown } from "../lib/visibility";
import type { SurfaceProps } from "../prototype/surfaces";
import {
  aspectRatio,
  clamp,
  contains,
  displayAt,
  fitAspect,
  growRect,
  MIN_SIZE,
  moveRect,
  rectFromDrag,
  resizeRect,
  setRectSize,
  snapTargets,
  targetAt,
  toPixels,
  toPoints,
  type Aspect,
  type Handle,
  type Pt,
  type Rect,
  type Size,
  type SnapTarget,
} from "./region-picker/geometry";
import {
  Crosshair,
  DimLayer,
  HANDLE_CURSOR,
  SelectionFrame,
  SnapHighlight,
  type CssRect,
} from "./region-picker/overlay-parts";
import { AppList, AreaOptions, DisplayList, WindowGrid } from "./region-picker/pickers";
import { RegionToolbar, type PickMode } from "./region-picker/toolbar";

const FALLBACK_DISPLAY: Display = {
  id: 1,
  name: "Built-in Retina Display",
  width: 3456,
  height: 2234,
  frame: { x: 0, y: 0, w: 1728, h: 1117 },
  scale: 2,
};
const inTauri = isTauri();

/** The display this overlay covers: the shell sizes the window to one whole display. */
function overlayDisplay(displays: readonly Display[]): Display {
  if (!inTauri) return displays.find((d) => d.id === 1) ?? FALLBACK_DISPLAY;
  return displayAt(displays, { x: window.screenX + 1, y: window.screenY + 1 }) ?? FALLBACK_DISPLAY;
}

type Drag =
  | { kind: "create"; anchor: Pt; origin: Pt; last: Pt; moved: boolean }
  | { kind: "move"; start: Rect; origin: Pt }
  | { kind: "resize"; start: Rect; handle: Handle };

function modeFor(t: CaptureTarget): PickMode {
  if (t.kind === "window") return "window";
  if (t.kind === "app") return "app";
  return "area";
}

/**
 * SOURCE OF TRUTH KEYWORDS: RegionPicker, region selection, area capture, window snapping, window picker, app picker, capture target
 * WHAT:  Full-display overlay for choosing what to record: draw/resize/move an area (with aspect
 *        locks, exact sizes and keyboard nudging), snap to a window, or pick a window, app or display.
 * WHY:   Rec only offered `--region` on the CLI and a window dropdown; Cortex adds a precise,
 *        visual picker (NEW-RG-01..04, NEW-WP-01, NEW-AP-02). In the real app this is one
 *        transparent, capture-excluded overlay window per display.
 * WHERE: Opened from the main window's source card; confirm writes `settings.target` and returns to Main.
 */
export function RegionPicker(props: SurfaceProps) {
  // The window is reused: each time it is shown, start over from the saved target, on the
  // display it now covers, with a fresh window list.
  const [opened, setOpened] = React.useState(0);
  React.useEffect(() => {
    engine.refreshDevices();
    return onPageShown(() => {
      engine.refreshDevices();
      setOpened((n) => n + 1);
    });
  }, []);
  return <RegionPickerBody key={opened} {...props} />;
}

function RegionPickerBody({ navigate }: SurfaceProps) {
  const e = useEngine();
  const words = usePlatformWords();
  const caps = e.capabilities;
  // With the system picker (Linux) the screen and window are chosen in the system dialog, so
  // only an area is drawn here; otherwise every kind the engine records.
  const supported: Record<PickMode, boolean> = {
    area: caps.regionCapture,
    window: caps.windowCapture && caps.sourcePicker === "app",
    app: caps.appCapture,
    display: caps.displayCapture && caps.sourcePicker === "app",
  };
  const modes = (["area", "window", "app", "display"] as const).filter((m) => supported[m]);
  const display = overlayDisplay(e.displays);
  const scale = display.scale > 0 ? display.scale : 1;
  // The picker works in display pixels (what gets recorded); the target is saved in points.
  const bounds: Size = { w: display.width, h: display.height };

  const rootRef = React.useRef<HTMLDivElement>(null);
  const [box, setBox] = React.useState<Size>({ w: 1, h: 1 });
  React.useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setBox({ w: entry.contentRect.width, h: entry.contentRect.height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Seeded once from the saved target so reopening the picker resumes where it was.
  const [saved] = React.useState(() => e.settings.target);
  const [mode, setMode] = React.useState<PickMode>(() => modeFor(saved));
  const [rect, setRect] = React.useState<Rect | null>(() => {
    const s = e.settings;
    const last = s.rememberRegion ? s.lastRegions.find((r) => r.displayId === display.id) : undefined;
    const seed = saved.kind === "region" && saved.displayId === display.id ? saved : last;
    return seed ? toPixels(seed, scale) : null;
  });
  const [snapId, setSnapId] = React.useState<number | null>(null);
  const [windowId, setWindowId] = React.useState<number | null>(() =>
    saved.kind === "window" ? saved.windowId : null,
  );
  const [bundleId, setBundleId] = React.useState<string | null>(() => (saved.kind === "app" ? saved.bundleId : null));
  const [displayId, setDisplayId] = React.useState(() => (saved.kind === "display" ? saved.displayId : display.id));
  const [aspect, setAspect] = React.useState<Aspect>("free");
  const [remember, setRemember] = React.useState(e.settings.rememberRegion);
  const [pointer, setPointer] = React.useState<Pt | null>(null);
  const [hover, setHover] = React.useState<SnapTarget | null>(null);
  const [drag, setDrag] = React.useState<Drag | null>(null);
  const spaceRef = React.useRef(false);

  const targets: SnapTarget[] = snapTargets(e.windows, display.id, scale);

  const sx = box.w / bounds.w;
  const sy = box.h / bounds.h;
  const toCss = (r: Rect): CssRect => ({ left: r.x * sx, top: r.y * sy, width: r.w * sx, height: r.h * sy });
  const toDisplay = (ev: React.PointerEvent): Pt => {
    const b = rootRef.current?.getBoundingClientRect();
    if (!b) return { x: 0, y: 0 };
    return { x: clamp((ev.clientX - b.left) / sx, 0, bounds.w), y: clamp((ev.clientY - b.top) / sy, 0, bounds.h) };
  };
  const ratioFor = (shift: boolean) => (shift ? 1 : aspectRatio(aspect));
  const appOf = (t: SnapTarget) => e.apps.find((a) => a.bundleId === t.bundleId);

  const selectedApp = e.apps.find((a) => a.bundleId === bundleId);
  const selectedWindow = e.windows.find((w) => w.id === windowId);
  const snapped = targets.find((t) => t.windowId === snapId);
  const validRect = rect !== null && rect.w >= MIN_SIZE && rect.h >= MIN_SIZE;

  // ---- pointer -------------------------------------------------------------
  function onPointerDown(ev: React.PointerEvent<HTMLDivElement>) {
    if (ev.button !== 0) return;
    const p = toDisplay(ev);
    if (mode === "area") {
      ev.currentTarget.setPointerCapture(ev.pointerId);
      if (rect && contains(rect, p)) setDrag({ kind: "move", start: rect, origin: p });
      else setDrag({ kind: "create", anchor: p, origin: p, last: p, moved: false });
      return;
    }
    const t = targetAt(targets, p);
    if (!t) return;
    if (mode === "window") setWindowId(t.windowId);
    if (mode === "app" && t.bundleId) setBundleId(t.bundleId);
  }

  function onHandleDown(h: Handle, ev: React.PointerEvent) {
    ev.stopPropagation();
    if (!rect || ev.button !== 0) return;
    rootRef.current?.setPointerCapture(ev.pointerId);
    setDrag({ kind: "resize", start: rect, handle: h });
  }

  function onPointerMove(ev: React.PointerEvent<HTMLDivElement>) {
    const p = toDisplay(ev);
    setPointer(p);
    if (!drag) {
      const overSelection = mode === "area" && rect !== null && contains(rect, p);
      setHover(mode === "display" || overSelection ? null : targetAt(targets, p));
      return;
    }
    setHover(null);
    if (drag.kind === "create") {
      const moved = drag.moved || Math.hypot((p.x - drag.origin.x) * sx, (p.y - drag.origin.y) * sy) > 4;
      // Space held: the whole rectangle travels with the pointer instead of growing.
      const anchor = spaceRef.current
        ? { x: drag.anchor.x + p.x - drag.last.x, y: drag.anchor.y + p.y - drag.last.y }
        : drag.anchor;
      if (moved) {
        setRect(rectFromDrag(anchor, p, ratioFor(ev.shiftKey), bounds));
        setSnapId(null);
      }
      setDrag({ ...drag, anchor, last: p, moved });
    } else if (drag.kind === "move") {
      setRect(moveRect(drag.start, p.x - drag.origin.x, p.y - drag.origin.y, bounds));
      setSnapId(null);
    } else {
      setRect(resizeRect(drag.start, drag.handle, p, ratioFor(ev.shiftKey), bounds));
      setSnapId(null);
    }
  }

  function onPointerUp(ev: React.PointerEvent<HTMLDivElement>) {
    if (!drag) return;
    if (ev.currentTarget.hasPointerCapture(ev.pointerId)) ev.currentTarget.releasePointerCapture(ev.pointerId);
    if (drag.kind === "create" && !drag.moved) {
      // A click (no drag) on a window snaps the selection to that window.
      const t = targetAt(targets, toDisplay(ev));
      if (t) {
        setRect(t.frame);
        setSnapId(t.windowId);
        setAspect("free");
      }
    } else if (rect && (rect.w < MIN_SIZE || rect.h < MIN_SIZE)) {
      setRect(null);
    }
    setDrag(null);
  }

  // ---- actions -------------------------------------------------------------
  function changeMode(m: PickMode) {
    setMode(m);
    setHover(null);
    if (m === "window" && snapId !== null) setWindowId(snapId);
  }

  function changeAspect(a: Aspect) {
    setAspect(a);
    const ratio = aspectRatio(a);
    if (rect && ratio) {
      setRect(fitAspect(rect, ratio, bounds));
      setSnapId(null);
    }
  }

  function changeSize(which: "w" | "h", n: number) {
    if (!rect) return;
    setRect(setRectSize(rect, which, n, aspectRatio(aspect), bounds));
    setSnapId(null);
  }

  function pendingTarget(): CaptureTarget | null {
    // A window clicked in Area mode is an area with that window's frame (Window mode
    // records the window itself).
    if (mode === "area") {
      return rect && validRect ? { kind: "region", displayId: display.id, ...toPoints(rect, scale) } : null;
    }
    if (mode === "window") return windowId === null ? null : { kind: "window", windowId };
    if (mode === "app") return bundleId === null ? null : { kind: "app", bundleId };
    return { kind: "display", displayId };
  }
  const target = pendingTarget();

  const confirm = () => {
    if (!target) return;
    engine.update({ target, rememberRegion: remember });
    navigate("main");
  };
  const cancel = () => navigate("main");

  // ---- keyboard ------------------------------------------------------------
  // Subscribed once with an effect event. Re-subscribing every render would drop keys, because
  // an engine update during the same keydown removes the listener mid-dispatch.
  const onKeyDown = React.useEffectEvent((ev: KeyboardEvent) => {
    const el = ev.target;
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
    // Enter on Cancel/confirm keeps its native click; on an option it still confirms.
    const onControl = el instanceof HTMLButtonElement && el.getAttribute("role") !== "radio";
    if (ev.key === "Escape") {
      ev.preventDefault();
      cancel();
    } else if (ev.key === "Enter" && !onControl) {
      ev.preventDefault();
      confirm();
    } else if (ev.key === " " && !onControl) {
      ev.preventDefault();
      spaceRef.current = true;
    } else if (mode === "area" && rect && ev.key.startsWith("Arrow")) {
      ev.preventDefault();
      const step = ev.shiftKey ? 10 : 1;
      const dx = ev.key === "ArrowLeft" ? -step : ev.key === "ArrowRight" ? step : 0;
      const dy = ev.key === "ArrowUp" ? -step : ev.key === "ArrowDown" ? step : 0;
      // ⌥ + arrows resize from the top-left corner; plain arrows nudge.
      setRect(ev.altKey ? growRect(rect, dx, dy, bounds) : moveRect(rect, dx, dy, bounds));
      setSnapId(null);
    }
  });
  React.useEffect(() => {
    const onKey = (ev: KeyboardEvent) => onKeyDown(ev);
    const onUp = (ev: KeyboardEvent) => {
      if (ev.key === " ") spaceRef.current = false;
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onUp);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onUp);
    };
  }, []);

  // ---- derived view --------------------------------------------------------
  const framesOf = (pred: (t: SnapTarget) => boolean) => targets.filter(pred);
  const chosenFrames =
    mode === "window"
      ? framesOf((t) => t.windowId === windowId)
      : mode === "app"
        ? framesOf((t) => t.bundleId === bundleId)
        : [];

  const holes =
    mode === "area"
      ? rect
        ? [{ ...toCss(rect), radius: snapped ? snapped.radius * rect.w * sx : 0 }]
        : []
      : chosenFrames.map((t) => ({ ...toCss(t.frame), radius: t.radius * t.frame.w * sx }));

  const adjusting = drag !== null && (drag.kind !== "create" || drag.moved);
  const showHover = hover !== null && !adjusting && !chosenFrames.some((t) => t.windowId === hover.windowId);

  const cursor =
    drag?.kind === "resize"
      ? HANDLE_CURSOR[drag.handle]
      : drag?.kind === "move"
        ? "move"
        : mode === "area"
          ? rect && pointer && contains(rect, pointer)
            ? "move"
            : "crosshair"
          : hover
            ? "pointer"
            : "default";

  const hoverHint =
    mode === "area"
      ? "Click to use this window · drag to draw an area"
      : mode === "window"
        ? "Click to record this window"
        : hover && appOf(hover)
          ? `Click to record every ${hover.app} window`
          : "This app isn't in the list yet";

  const primaryLabel =
    mode === "area"
      ? "Record area"
      : mode === "window"
        ? "Use window"
        : mode === "app"
          ? "Use app"
          : "Record full screen";

  const hint =
    mode === "area"
      ? rect
        ? `Drag inside to move · Arrows nudge 1 px (Shift 10, ${words.altKey} resizes) · ↵ to confirm · Esc to cancel`
        : "Drag to select · Shift for square · Space to move · Esc to cancel"
      : mode === "window"
        ? "Click a window on screen or in the list · Esc to cancel"
        : mode === "app"
          ? "Click any window to pick its app · Esc to cancel"
          : "↵ to record the whole display · Esc to cancel";

  // Window or app chosen but none of its windows are on this display.
  const offscreenNote =
    mode === "window" && selectedWindow && chosenFrames.length === 0
      ? `${selectedWindow.app} — ${selectedWindow.title} isn't on this display. It's still recorded wherever it is.`
      : mode === "app" && selectedApp && chosenFrames.length === 0
        ? `No ${selectedApp.name} windows on this display. They're recorded on any display.`
        : mode === "display" && displayId !== display.id
          ? `${e.displays.find((d) => d.id === displayId)?.name ?? "That display"} is outlined on its own screen.`
          : null;

  return (
    <div
      ref={rootRef}
      role="application"
      aria-label="Select what to record"
      className="absolute inset-0 z-50 touch-none overflow-hidden select-none"
      style={{ cursor }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => setDrag(null)}
      onPointerLeave={() => {
        if (!drag) {
          setPointer(null);
          setHover(null);
        }
      }}
    >
      {mode === "display" && displayId === display.id ? (
        <div aria-hidden className="pointer-events-none absolute inset-0 ring-[3px] ring-accent-solid ring-inset">
          <div className="absolute top-1/2 left-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center gap-2.5 rounded-[14px] bg-popover px-4 py-2.5 text-popover-foreground shadow-float">
            <span className="text-[13px] font-semibold">{display.name}</span>
            <span className="tabular text-[13px] text-muted-foreground">
              {display.width} × {display.height}
            </span>
          </div>
        </div>
      ) : (
        <DimLayer holes={holes} />
      )}

      {mode === "area" && !rect && !drag && pointer && (
        <Crosshair x={pointer.x * sx} y={pointer.y * sy} label={`${Math.round(pointer.x)}, ${Math.round(pointer.y)}`} />
      )}

      {showHover && hover && (
        <SnapHighlight
          rect={toCss(hover.frame)}
          radius={hover.radius * hover.frame.w * sx}
          app={hover.app}
          bundleId={hover.bundleId}
          title={hover.title}
          hint={hoverHint}
        />
      )}

      {mode === "area" && rect && (
        <SelectionFrame
          rect={toCss(rect)}
          sizeLabel={`${rect.w} × ${rect.h}`}
          detail={
            drag?.kind === "move"
              ? `at ${rect.x}, ${rect.y}`
              : snapped
                ? snapped.title
                : aspect !== "free"
                  ? aspect
                  : undefined
          }
          snappedApp={snapped?.app}
          snappedBundleId={snapped?.bundleId}
          radius={snapped ? snapped.radius * rect.w * sx : 0}
          adjusting={adjusting}
          editable={!adjusting || drag?.kind !== "move"}
          containerHeight={box.h}
          onHandleDown={onHandleDown}
        />
      )}

      {mode !== "area" &&
        mode !== "display" &&
        chosenFrames.map((t) => (
          <SelectionFrame
            key={t.windowId}
            rect={toCss(t.frame)}
            sizeLabel={`${t.frame.w} × ${t.frame.h}`}
            detail={t.title}
            snappedApp={t.app}
            snappedBundleId={t.bundleId}
            adjusting={false}
            editable={false}
            radius={t.radius * t.frame.w * sx}
            containerHeight={box.h}
            onHandleDown={() => {}}
          />
        ))}

      {offscreenNote && (
        <div className="pointer-events-none absolute top-[38%] left-1/2 flex max-w-md -translate-x-1/2 items-center gap-2.5 rounded-[14px] bg-popover px-4 py-3 text-[13px] text-popover-foreground shadow-float">
          <AppWindow className="size-4 shrink-0 text-muted-foreground" />
          {offscreenNote}
        </div>
      )}

      {mode === "area" && !drag && (
        <CoachMark hint="area-picker" title="Drawing an area" className="absolute top-[14%] left-1/2 -translate-x-1/2">
          Drag to draw, or click a window to snap to its frame. Hold Shift for a square, Space to move it, and press
          Return to record.
        </CoachMark>
      )}

      <RegionToolbar
        mode={mode}
        modes={modes}
        onMode={changeMode}
        primaryLabel={primaryLabel}
        canConfirm={target !== null}
        onConfirm={confirm}
        onCancel={cancel}
        hint={hint}
        receded={adjusting}
        onPointerEnter={() => {
          if (drag) return;
          setPointer(null);
          setHover(null);
        }}
      >
        {mode === "area" && (
          <AreaOptions
            aspect={aspect}
            onAspect={changeAspect}
            size={rect ? { w: rect.w, h: rect.h } : null}
            onSize={changeSize}
            remember={remember}
            onRemember={setRemember}
          />
        )}
        {mode === "window" && (
          <WindowGrid
            windows={e.windows}
            selectedId={windowId}
            onSelect={setWindowId}
            onRefresh={() => {
              setHover(null);
              engine.refreshDevices();
            }}
          />
        )}
        {mode === "app" && <AppList apps={e.apps} selectedId={bundleId} onSelect={setBundleId} />}
        {mode === "display" && (
          <DisplayList displays={e.displays} selectedId={displayId} currentId={display.id} onSelect={setDisplayId} />
        )}
      </RegionToolbar>
    </div>
  );
}
