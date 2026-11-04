import type * as React from "react";
import {
  AppWindow,
  Camera,
  Check,
  Ellipsis,
  FolderOpen,
  History,
  Loader2,
  MousePointerClick,
  Mic,
  Pause,
  Play,
  Radio,
  Scissors,
  Settings,
  Square,
  TriangleAlert,
  Volume2,
} from "lucide-react";

import { Button, buttonVariants } from "@starter/ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@starter/ui/components/dropdown-menu";
import { Kbd } from "@starter/ui/components/kbd";
import { LevelMeter, type LevelSource } from "@starter/ui/components/level-meter";
import { Segmented } from "@starter/ui/components/segmented";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { BrandMark } from "../components/brand-mark";
import { BytesWritten, DroppedFrames, ElapsedClock } from "../components/progress-text";
import { formatAgo, formatBytes, formatClock } from "../lib/format";
import { ShortcutKbd, useShortcut } from "../components/shortcut-kbd";
import { engine, micLevels, systemLevels, useEngineCalm } from "../lib/engine";
import { usePlatformWords } from "../lib/platform";
import type { EngineState } from "../lib/types";
import { checkForUpdates, useUpdates } from "../lib/updates";
import { MENUBAR_H } from "../prototype/desktop-backdrop";
import type { SurfaceProps } from "../prototype/surfaces";
import { ClipThumb } from "./floating/clip-thumb";
import { FloatingCard, MENU_CONTENT, MENU_ITEM } from "./floating/floating-card";
import { isCapturing, LiveBadge, RecStatus } from "./floating/status";
import { useFlash } from "./floating/use-flash";

type QuickTarget = "screen" | "window" | "area";

/**
 * SOURCE OF TRUTH KEYWORDS: MenuBarPanel, status item panel, tray panel, quick capture, MNU, menubar
 * WHAT:  The panel that drops from Cortex's menu-bar item: quick capture setup and Start when
 *        idle; clock, meters, Pause/Stop and replay while recording; last recording; app links.
 * WHY:   Most recordings start and stop from the menu bar without opening the main window.
 *        It carries every item of Rec's status menu (MNU-02…10) in a richer, glanceable form.
 * WHERE: Prototype surface "menubar"; later a non-activating NSPanel anchored to the status item.
 */
export function MenuBarPanel({ navigate }: SurfaceProps) {
  const quitKeys = useShortcut("quit");
  const e = useEngineCalm();
  const capturing = isCapturing(e.status);

  return (
    <FloatingCard
      role="dialog"
      aria-label="Cortex menu bar panel"
      className="absolute w-[340px] origin-top-right overflow-hidden duration-200 ease-out-quick animate-in fade-in zoom-in-[0.97] slide-in-from-top-1"
      style={{ top: MENUBAR_H + 6, right: 150 }}
    >
      <header className="flex h-11 items-center gap-2 px-3.5">
        <BrandMark className="size-4" />
        <span className="text-[13px] font-semibold">Cortex</span>
        <RecStatus status={e.status} />
        {e.live && <LiveBadge />}
        <PanelMenu e={e} navigate={navigate} />
      </header>

      <div className="space-y-3 px-3 pb-3">
        {e.status === "idle" && <IdleBody e={e} navigate={navigate} />}
        {e.status === "countdown" && <CountdownBody left={e.countdownLeft} />}
        {(capturing || e.status === "finalizing") && <RecordingBody e={e} />}
      </div>

      <footer className="flex items-center gap-1 border-t px-2 py-1.5">
        <Button variant="ghost" size="sm" className="text-accent-text" onClick={() => navigate("main")}>
          <AppWindow />
          Open Cortex
        </Button>
        <Button variant="ghost" size="sm" onClick={() => navigate("settings")}>
          <Settings />
          Settings
          <ShortcutKbd action="settings" className="ml-0.5 h-4 min-w-4 px-1 text-[10px] shadow-none" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto text-muted-foreground"
          // Quit finalizes an active recording first (KEY-05); the prototype stops it.
          onClick={() => engine.stop()}
          title={`Quit Cortex (${quitKeys}). Saves an active recording first.`}
        >
          Quit
        </Button>
      </footer>
    </FloatingCard>
  );
}

/* ── Idle ─────────────────────────────────────────────────────────────── */

function IdleBody({ e, navigate }: { e: EngineState; navigate: SurfaceProps["navigate"] }) {
  const words = usePlatformWords();
  const s = e.settings;
  const target: QuickTarget = s.target.kind === "display" ? "screen" : s.target.kind === "region" ? "area" : "window";
  const last = e.recordings[0];
  const blocked = e.permissions.screen !== "granted";
  const camera = e.cameras.find((c) => c.id === s.cameraId);
  const mic = e.mics.find((m) => m.id === s.micId);

  function pickTarget(next: QuickTarget) {
    if (next === "area") return navigate("region");
    if (next === "screen") return engine.update({ target: { kind: "display", displayId: e.displays[0]?.id ?? 1 } });
    const win = e.windows[0];
    if (win) engine.update({ target: { kind: "window", windowId: win.id } });
  }

  return (
    <>
      <div>
        <Segmented<QuickTarget>
          aria-label="What to record"
          className="w-full"
          value={target}
          onValueChange={pickTarget}
          options={[
            { value: "screen", label: "Screen" },
            { value: "window", label: "Window" },
            { value: "area", label: "Area" },
          ]}
        />
        <div className="mt-1.5 flex items-center gap-1 px-1 text-xs text-muted-foreground">
          <span className="min-w-0 truncate">{targetLabel(e)}</span>
          {target !== "screen" && (
            <button
              type="button"
              className="shrink-0 rounded-[4px] font-medium text-accent-text outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50"
              onClick={() => navigate("region")}
            >
              Change…
            </button>
          )}
        </div>
      </div>

      <div className="divide-y divide-border/70 rounded-[14px] bg-muted px-3">
        <ToggleRow
          icon={<Camera />}
          label="Camera"
          detail={camera?.name ?? "No camera"}
          checked={s.scene !== "screen"}
          onChange={(on) => engine.update({ scene: on ? "screen-cam" : "screen" })}
        />
        <ToggleRow
          icon={<Mic />}
          label="Microphone"
          detail={mic?.name ?? "No microphone"}
          checked={s.micEnabled}
          onChange={(micEnabled) => engine.update({ micEnabled })}
        />
        <ToggleRow
          icon={<Volume2 />}
          label="System audio"
          detail="Sound from apps"
          checked={s.systemAudio}
          onChange={(systemAudio) => engine.update({ systemAudio })}
        />
        <ToggleRow
          icon={<MousePointerClick />}
          label="Highlight clicks"
          detail="Ring around each click"
          checked={s.clicks}
          onChange={(clicks) => engine.update({ clicks })}
        />
      </div>

      {blocked ? (
        <div className="flex items-start gap-2.5 rounded-[14px] bg-warn-tint p-3 text-[13px]">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warn" aria-hidden />
          <div className="min-w-0 flex-1">
            <div className="font-medium">Screen Recording is off</div>
            <div className="text-xs text-muted-foreground">Allow Cortex in {words.settingsApp} to record.</div>
          </div>
          <Button variant="outline" size="xs" onClick={() => navigate("onboarding")}>
            Allow…
          </Button>
        </div>
      ) : null}

      <div>
        <Button
          variant="record"
          size="xl"
          className="w-full justify-start"
          disabled={blocked}
          onClick={() => engine.start()}
        >
          <span aria-hidden className="grid size-4 place-items-center rounded-full ring-2 ring-rec-fg/90">
            <span className="size-2 rounded-full bg-rec-fg" />
          </span>
          Start recording
          <ShortcutKbd action="record" className="ml-auto border-transparent bg-rec-fg/15 text-rec-fg shadow-none" />
        </Button>
        <div className="mt-1.5 px-1 text-xs text-muted-foreground tabular">
          {s.countdown > 0 ? `${s.countdown} s countdown · ` : ""}
          {s.resolution === "native" ? "Native size" : s.resolution} · {s.fps} fps · {s.container.toUpperCase()}
        </div>
      </div>

      {last ? <LastRecording e={e} navigate={navigate} /> : null}
    </>
  );
}

function ToggleRow({
  icon,
  label,
  detail,
  checked,
  onChange,
}: {
  icon: React.ReactNode;
  label: string;
  detail: string;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <label className="flex h-11 cursor-default items-center gap-2.5">
      <span
        aria-hidden
        className={cn(
          "grid size-[26px] shrink-0 place-items-center rounded-[6px] bg-card shadow-ledge transition-colors [&_svg]:size-3.5",
          checked ? "text-accent-text" : "text-muted-foreground",
        )}
      >
        {icon}
      </span>
      <span className="min-w-0 flex-1 leading-tight">
        <span className="block text-[13px] font-medium">{label}</span>
        <span className="block truncate text-[11.5px] text-muted-foreground" title={detail}>
          {detail}
        </span>
      </span>
      <Switch
        size="sm"
        className="data-unchecked:bg-foreground/15"
        checked={checked}
        onCheckedChange={onChange}
        aria-label={label}
      />
    </label>
  );
}

function LastRecording({ e, navigate }: { e: EngineState; navigate: SurfaceProps["navigate"] }) {
  const words = usePlatformWords();
  const [flashed, flash] = useFlash();
  const last = e.recordings[0];
  if (!last) return null;
  return (
    <div className="rounded-[14px] border p-2.5">
      <div className="mb-2 px-0.5 text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
        Last recording
      </div>
      <div className="flex items-center gap-3">
        <ClipThumb durationSecs={last.durationSecs} width={80} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-medium" title={last.name}>
            {last.name}
          </div>
          <div className="text-xs text-muted-foreground tabular">
            {formatClock(last.durationSecs)} · {formatBytes(last.bytes)} · {formatAgo(last.createdAt)}
          </div>
          <div className="mt-1.5 flex gap-1">
            <Button
              variant="soft"
              size="xs"
              onClick={() => flash("reveal")}
              title={`Show ${last.path} in ${words.fileManager}`}
            >
              {flashed === "reveal" ? <Check /> : <FolderOpen />}
              {flashed === "reveal" ? "Shown" : "Reveal"}
            </Button>
            {e.capabilities.quickTrim && (
              <Button variant="soft" size="xs" onClick={() => navigate("trim")}>
                <Scissors />
                Trim
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

/* ── Countdown / recording ────────────────────────────────────────────── */

function CountdownBody({ left }: { left: number }) {
  return (
    <div className="flex items-center gap-4 rounded-[14px] bg-muted p-4">
      <span
        key={left}
        className="font-display text-[44px] leading-none font-bold tabular duration-300 ease-spring animate-in zoom-in-125"
      >
        {left}
      </span>
      <div className="min-w-0 flex-1 text-[13px] whitespace-nowrap">
        <div className="font-medium">Recording in {left} s</div>
        <div className="text-xs text-muted-foreground">Nothing captured yet</div>
      </div>
      <Button variant="outline" size="sm" onClick={() => engine.cancelCountdown()}>
        Cancel
        <Kbd className="h-4 min-w-4 px-1 text-[10px] shadow-none">Esc</Kbd>
      </Button>
    </div>
  );
}

function RecordingBody({ e }: { e: EngineState }) {
  const recordKeys = useShortcut("record");
  const replayKeys = useShortcut("replay");
  const s = e.settings;
  const paused = e.status === "paused";
  const saving = e.status === "finalizing";
  return (
    <>
      <div className="rounded-[14px] bg-muted p-4">
        <div className="flex items-center justify-between text-xs text-muted-foreground tabular">
          <span className="min-w-0 truncate">{targetLabel(e)}</span>
          <span className="shrink-0">
            <BytesWritten />
          </span>
        </div>
        <div
          className={cn(
            "mt-1 text-[46px] leading-none font-semibold tracking-tight tabular transition-colors",
            paused || saving ? "text-muted-foreground" : "text-foreground",
          )}
          aria-live="off"
        >
          <ElapsedClock />
        </div>
        <div className="mt-1 text-xs text-muted-foreground tabular">
          {s.resolution === "native" ? "Native" : s.resolution} · {s.fps} fps · <DroppedFrames />
        </div>
        <div className="mt-3 space-y-1.5">
          <MeterLine icon={<Volume2 />} label="System" on={s.systemAudio && !s.systemMuted} levels={systemLevels} />
          <MeterLine icon={<Mic />} label="Mic" on={s.micEnabled && !s.micMuted} levels={micLevels} />
        </div>
      </div>

      {saving ? (
        <div className="flex h-11 items-center justify-center gap-2 rounded-[14px] bg-muted text-[13px] text-muted-foreground">
          <Loader2 className="size-4 animate-spin" aria-hidden />
          Saving recording…
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-2">
          <Button variant="soft" size="xl" onClick={() => engine.togglePause()}>
            {paused ? <Play /> : <Pause />}
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button variant="record" size="xl" onClick={() => engine.stop()} title={`Stop and save (${recordKeys})`}>
            <Square className="fill-current" />
            Stop
            <ShortcutKbd action="record" className="border-transparent bg-rec-fg/15 text-rec-fg shadow-none" />
          </Button>
        </div>
      )}

      {s.replayEnabled && e.capabilities.replayBuffer && !saving ? (
        <div className="flex items-center gap-2.5 rounded-[14px] border px-3 py-2">
          <History className="size-4 shrink-0 text-muted-foreground" aria-hidden />
          <div className="min-w-0 flex-1 text-xs text-muted-foreground">
            Replay buffer keeps the last {s.replaySecs} s
          </div>
          <Button
            variant="soft"
            size="xs"
            onClick={() => engine.saveReplay()}
            title={`Save the buffer to an MP4 without stopping (${replayKeys})`}
          >
            Save last {s.replaySecs}s
          </Button>
        </div>
      ) : null}
    </>
  );
}

function MeterLine({
  icon,
  label,
  on,
  levels,
}: {
  icon: React.ReactNode;
  label: string;
  on: boolean;
  levels: LevelSource;
}) {
  return (
    <div className="flex items-center gap-2 text-[11.5px] text-muted-foreground">
      <span aria-hidden className="[&_svg]:size-3.5">
        {icon}
      </span>
      <span className="w-11">{label}</span>
      <LevelMeter aria-label={`${label} level`} className="h-1.5 flex-1" segments={20} source={levels} muted={!on} />
    </div>
  );
}

/* ── Overflow menu: the remaining status-menu items (MNU-02…10) ───────── */

function PanelMenu({ e, navigate }: { e: EngineState; navigate: SurfaceProps["navigate"] }) {
  const recordKeys = useShortcut("record");
  const replayKeys = useShortcut("replay");
  const settingsKeys = useShortcut("settings");
  const capturing = isCapturing(e.status);
  const update = useUpdates().available;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label="More actions"
        className={cn(buttonVariants({ variant: "ghost", size: "icon-sm" }), "ml-auto text-muted-foreground")}
      >
        <Ellipsis />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className={MENU_CONTENT}>
        <DropdownMenuItem
          className={MENU_ITEM}
          disabled={e.status === "finalizing"}
          onClick={() => engine.toggleRecord()}
        >
          {e.status === "idle" ? "Start recording" : e.status === "countdown" ? "Cancel countdown" : "Stop recording"}
          <DropdownMenuShortcut>{recordKeys}</DropdownMenuShortcut>
        </DropdownMenuItem>
        <DropdownMenuItem className={MENU_ITEM} disabled={!capturing} onClick={() => engine.togglePause()}>
          {e.status === "paused" ? "Resume" : "Pause"}
        </DropdownMenuItem>
        <DropdownMenuItem className={MENU_ITEM} disabled={!capturing} onClick={() => navigate("hud")}>
          Show recording HUD
        </DropdownMenuItem>
        {e.capabilities.replayBuffer && (
          <DropdownMenuItem
            className={MENU_ITEM}
            disabled={!e.settings.replayEnabled}
            onClick={() => engine.saveReplay()}
          >
            Save replay
            <DropdownMenuShortcut>{replayKeys}</DropdownMenuShortcut>
          </DropdownMenuItem>
        )}
        {e.capabilities.liveStreaming && (
          <DropdownMenuItem className={MENU_ITEM} onClick={() => engine.toggleLive()}>
            <Radio />
            {e.live ? "Stop streaming" : "Go live"}
          </DropdownMenuItem>
        )}
        <DropdownMenuSeparator className="mx-1 my-1" />
        <DropdownMenuItem className={MENU_ITEM} disabled={e.recordings.length === 0}>
          Reveal last recording
        </DropdownMenuItem>
        <DropdownMenuItem className={MENU_ITEM}>Open recordings folder</DropdownMenuItem>
        <DropdownMenuItem className={MENU_ITEM} onClick={() => navigate("settings")}>
          Settings…
          <DropdownMenuShortcut>{settingsKeys}</DropdownMenuShortcut>
        </DropdownMenuItem>
        <DropdownMenuItem
          className={MENU_ITEM}
          onClick={() => {
            // The result shows in Settings → General, which also installs.
            if (!update) checkForUpdates();
            navigate("settings");
          }}
        >
          {update ? `Update to Cortex ${update.version}…` : "Check for updates…"}
        </DropdownMenuItem>
        <DropdownMenuSeparator className="mx-1 my-1" />
        <DropdownMenuItem className={cn(MENU_ITEM, "text-xs")} disabled>
          Global shortcut: {recordKeys}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function targetLabel(e: EngineState) {
  const t = e.settings.target;
  switch (t.kind) {
    case "display": {
      const d = e.displays.find((x) => x.id === t.displayId);
      return d ? `${d.name} · ${d.width}×${d.height}` : "Main display";
    }
    case "window": {
      const w = e.windows.find((x) => x.id === t.windowId);
      return w ? `${w.app} — ${w.title}` : "A window";
    }
    case "app": {
      const a = e.apps.find((x) => x.bundleId === t.bundleId);
      return a ? `All ${a.name} windows` : "An app";
    }
    case "region":
      return `Area · ${t.w}×${t.h}`;
  }
}
