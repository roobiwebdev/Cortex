import * as React from "react";
import {
  ArrowLeftToLine,
  ArrowRightToLine,
  Pause,
  Play,
  Scissors,
  Volume1,
  Volume2,
  VolumeX,
  ZoomIn,
  ZoomOut,
} from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { Slider } from "@starter/ui/components/slider";
import { cn } from "@starter/ui/lib/utils";

import { formatAgo, formatBytes } from "../lib/format";
import { engine, useEngine } from "../lib/mock-engine";
import type { RecorderSettings, Recording } from "../lib/types";
import type { SurfaceProps } from "../prototype/surfaces";
import { type ExportMode, type ExportOutput, type ExportPhase, TrimExportBar } from "./trim/export-bar";
import { type PlayerView, TrimPlayer } from "./trim/player";
import { ReplaceSheet } from "./trim/replace-sheet";
import {
  clamp,
  easeInOutCubic,
  formatShortClock,
  formatTimecode,
  MIN_SELECTION,
  snapToFrame,
  TRIM_FPS,
} from "./trim/time";
import { TrimTimeline } from "./trim/timeline";
import { EditableTitle } from "./trim/title-name";
import { type KeptTracks, TrimTracksPanel } from "./trim/tracks-panel";
import { usePlayback } from "./trim/use-playback";

/**
 * SOURCE OF TRUTH KEYWORDS: TrimWindow, quick trim, trim editor, in/out points, export trimmed copy
 * WHAT:  The Quick trim window: preview the latest recording, set in/out on a filmstrip
 *        timeline, choose which tracks to keep, and export fast (passthrough) or exact.
 * WHY:   Most recordings only need their dead start and end cut. Doing that here, without a
 *        full editor, is the MVP trim feature (NEW-TR-01..06, NEW-CT-01/02).
 * WHERE: Opened from "Reveal last" / after stop; prototype surface id "trim".
 */
export function TrimWindow({ cameraStream, navigate }: SurfaceProps) {
  const e = useEngine();
  const latest = React.useMemo(() => e.recordings.toSorted((a, b) => b.createdAt - a.createdAt)[0], [e.recordings]);

  if (!latest) {
    return (
      <div className="grid size-full place-items-center pt-11 text-center">
        <div>
          <Scissors className="mx-auto size-6 text-muted-foreground" />
          <p className="mt-3 text-[13px] font-medium">Nothing to trim yet</p>
          <p className="mt-1 text-[12px] text-muted-foreground">Your next recording opens here.</p>
        </div>
      </div>
    );
  }
  // Remount per recording so in/out and playback reset.
  return (
    <TrimEditor
      key={latest.id}
      recording={latest}
      settings={e.settings}
      micName={e.mics.find((m) => m.id === e.settings.micId)?.name ?? "Microphone"}
      cameraStream={cameraStream}
      onClose={() => navigate("main")}
    />
  );
}

function TrimEditor({
  recording: rec,
  settings,
  micName,
  cameraStream,
  onClose,
}: {
  recording: Recording;
  settings: RecorderSettings;
  micName: string;
  cameraStream: MediaStream | null;
  onClose: () => void;
}) {
  const duration = rec.durationSecs;
  const [inT, setInT] = React.useState(0);
  const [outT, setOutT] = React.useState(duration);
  const pb = usePlayback({ duration, inT, outT });
  const [zoom, setZoom] = React.useState(1);
  const [volume, setVolume] = React.useState(0.8);
  const [muted, setMuted] = React.useState(false);
  const [view, setView] = React.useState<PlayerView>("composite");
  // Prototype-local: which tracks the export keeps (no engine field yet).
  const [kept, setKept] = React.useState<KeptTracks>({ camera: true, system: true, mic: true });
  const [mode, setMode] = React.useState<ExportMode>("fast");
  const [phase, setPhase] = React.useState<ExportPhase>({ kind: "idle" });
  const [confirmReplace, setConfirmReplace] = React.useState(false);
  const exportRaf = React.useRef(0);

  const keep = outT - inT;
  const keepFrac = duration > 0 ? keep / duration : 1;
  const trimmed = inT > 0 || outT < duration;
  const tracksChanged = (rec.hasCameraTrack && !kept.camera) || !kept.system || !kept.mic;
  const busy = phase.kind === "exporting";
  const ext = rec.path.slice(rec.path.lastIndexOf(".")) || ".mov";
  const copyName = `${rec.name}-trim${ext}`;
  const originalName = `${rec.name}${ext}`;
  const folder = rec.path.slice(0, rec.path.lastIndexOf("/"));
  // Rough size: video dominates; the raw camera track is about a fifth of the file.
  const estBytes =
    rec.bytes * keepFrac * (rec.hasCameraTrack && !kept.camera ? 0.8 : 1) * (kept.system || kept.mic ? 1 : 0.97);

  const setIn = React.useCallback((t: number) => setInT(snapToFrame(clamp(t, 0, outT - MIN_SELECTION))), [outT]);
  const setOut = React.useCallback(
    (t: number) => setOutT(snapToFrame(clamp(t, inT + MIN_SELECTION, duration))),
    [inT, duration],
  );

  // Simulated export: eased 0→100%, faster for passthrough than for re-encode.
  function startExport(output: ExportOutput) {
    setConfirmReplace(false);
    pb.pause();
    const total = mode === "fast" ? 1.4 + keep / 400 : clamp(2.8 + keep / 45, 3, 9);
    const t0 = performance.now();
    const tick = (now: number) => {
      const x = clamp((now - t0) / 1000 / total, 0, 1);
      if (x >= 1) {
        setPhase({
          kind: "done",
          output,
          fileName: output === "copy" ? copyName : originalName,
          detail: `${formatShortClock(keep)} · ${formatBytes(estBytes)} · ${folder}`,
        });
        return;
      }
      setPhase({
        kind: "exporting",
        output,
        progress: easeInOutCubic(x),
        secsLeft: Math.ceil(total * (1 - x)),
      });
      exportRaf.current = requestAnimationFrame(tick);
    };
    exportRaf.current = requestAnimationFrame(tick);
  }
  function stopExport() {
    cancelAnimationFrame(exportRaf.current);
    setPhase({ kind: "idle" });
  }
  React.useEffect(() => () => cancelAnimationFrame(exportRaf.current), []);

  // Window shortcuts: Space, J/K/L, I/O, ←/→ frame (⇧ = 1 s), Home/End jump to in/out.
  React.useEffect(() => {
    if (busy || confirmReplace || phase.kind === "done") return;
    const onKey = (ev: KeyboardEvent) => {
      const target = ev.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
      if (ev.metaKey || ev.ctrlKey || ev.altKey) return;
      const k = ev.key.toLowerCase();
      let handled = true;
      if (k === " ") pb.toggle();
      else if (k === "k") pb.pause();
      else if (k === "l") pb.forward();
      else if (k === "j") pb.backward();
      else if (k === "i") setIn(pb.time);
      else if (k === "o") setOut(pb.time);
      else if (k === "arrowleft" || k === "arrowright") {
        pb.pause();
        const d = (ev.shiftKey ? 1 : 1 / TRIM_FPS) * (k === "arrowleft" ? -1 : 1);
        pb.seek(snapToFrame(pb.time + d));
      } else if (k === "home") pb.seek(inT);
      else if (k === "end") pb.seek(outT);
      else handled = false;
      if (handled) ev.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pb, setIn, setOut, inT, outT, busy, confirmReplace, phase.kind]);

  const VolumeIcon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  return (
    <div className="relative flex size-full flex-col">
      {/* Title bar zone (traffic lights are drawn by the window at left). */}
      <header className="relative flex h-11 shrink-0 items-center justify-center border-b">
        <EditableTitle value={rec.name} onCommit={(name) => engine.renameRecording(rec.id, name)} />
        <p className="absolute right-4 text-[11.5px] text-muted-foreground tabular">
          {rec.width}×{rec.height} · {formatBytes(rec.bytes)} · {formatAgo(rec.createdAt)}
        </p>
      </header>

      <div className={cn("flex min-h-0 flex-1 flex-col", busy && "pointer-events-none")}>
        {/* Player + tracks */}
        <div className="flex min-h-0 flex-1">
          <div className="relative min-w-0 flex-1 bg-stage p-4">
            <TrimPlayer
              settings={settings}
              view={view}
              playing={pb.playing}
              cameraStream={cameraStream}
              onToggle={pb.toggle}
              aspect={rec.width / rec.height}
            />
          </div>
          <aside className={cn("w-[244px] shrink-0 border-l bg-muted/60 transition-opacity", busy && "opacity-50")}>
            <TrimTracksPanel
              hasCameraTrack={rec.hasCameraTrack}
              micName={micName}
              kept={kept}
              onKeptChange={setKept}
              view={view}
              onViewChange={setView}
              disabled={busy}
            />
          </aside>
        </div>

        <div className={cn("flex shrink-0 flex-col transition-opacity", busy && "opacity-50")}>
          {/* Transport */}
          <div className="flex h-12 shrink-0 items-center gap-1 border-t px-3">
            <IconButton label="Go to start of selection (Home)" onClick={() => pb.seek(inT)}>
              <ArrowLeftToLine />
            </IconButton>
            <Button
              variant="soft"
              size="icon"
              className="mx-0.5 size-9 rounded-[10px]"
              aria-label={pb.playing ? "Pause (Space)" : "Play (Space)"}
              title={pb.playing ? "Pause (Space)" : "Play (Space)"}
              onClick={pb.toggle}
            >
              {pb.playing ? <Pause className="fill-current" /> : <Play className="translate-x-px fill-current" />}
            </Button>
            <IconButton label="Go to end of selection (End)" onClick={() => pb.seek(outT)}>
              <ArrowRightToLine />
            </IconButton>
            <p className="ml-2 font-mono text-[13px] tabular" aria-label="Current time">
              <span>{formatTimecode(pb.time)}</span>
              <span className="text-muted-foreground"> / {formatTimecode(duration)}</span>
            </p>
            {pb.rate !== 0 && pb.rate !== 1 && (
              <span className="ml-1 rounded-[6px] bg-accent-tint px-1.5 py-0.5 font-mono text-[11px] text-accent-text tabular">
                {pb.rate < 0 ? "◀ " : ""}
                {Math.abs(pb.rate)}×
              </span>
            )}

            <div className="ml-auto flex items-center gap-3 text-[11.5px] text-muted-foreground">
              <span className="flex items-center gap-1">
                <Kbd>J</Kbd>
                <Kbd>K</Kbd>
                <Kbd>L</Kbd>
                <span className="ml-0.5">shuttle</span>
              </span>
              <span className="flex items-center gap-1">
                <Kbd>I</Kbd>
                <Kbd>O</Kbd>
                <span className="ml-0.5">set start / end</span>
              </span>
              <span className="flex items-center gap-1">
                <Kbd>←</Kbd>
                <Kbd>→</Kbd>
                <span className="ml-0.5">frame</span>
              </span>
            </div>

            <div className="ml-5 flex w-[132px] items-center gap-2">
              <IconButton label={muted ? "Unmute" : "Mute"} onClick={() => setMuted(!muted)}>
                <VolumeIcon />
              </IconButton>
              <Slider
                aria-label="Volume"
                min={0}
                max={1}
                step={0.01}
                value={[muted ? 0 : volume]}
                onValueChange={([v]) => {
                  if (v === undefined) return;
                  setVolume(v);
                  setMuted(v === 0);
                }}
              />
            </div>
          </div>

          {/* Selection readout + zoom */}
          <div className="flex h-8 shrink-0 items-center gap-4 px-4 text-[11.5px]">
            <Readout label="Start" value={formatTimecode(inT)} />
            <Readout label="End" value={formatTimecode(outT)} />
            <span className="flex items-baseline gap-1.5">
              <span className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
                Keeps
              </span>
              <span className="font-mono text-[12px] font-medium text-accent-text tabular">{formatTimecode(keep)}</span>
              {trimmed && (
                <span className="text-muted-foreground tabular">· cuts {formatShortClock(duration - keep)}</span>
              )}
            </span>
            {trimmed && (
              <button
                type="button"
                onClick={() => {
                  setInT(0);
                  setOutT(duration);
                }}
                className="rounded-[6px] px-1.5 py-0.5 text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                Reset
              </button>
            )}
            <div className="ml-auto flex w-[180px] items-center gap-2 text-muted-foreground">
              <IconButton label="Zoom out" onClick={() => setZoom((z) => clamp(z / 1.5, 1, 8))}>
                <ZoomOut />
              </IconButton>
              <Slider
                aria-label="Timeline zoom"
                min={1}
                max={8}
                step={0.1}
                value={[zoom]}
                onValueChange={([z]) => z !== undefined && setZoom(z)}
              />
              <IconButton label="Zoom in" onClick={() => setZoom((z) => clamp(z * 1.5, 1, 8))}>
                <ZoomIn />
              </IconButton>
            </div>
          </div>

          <div className="shrink-0 px-1">
            <TrimTimeline
              recordingId={rec.id}
              duration={duration}
              inT={inT}
              outT={outT}
              time={pb.time}
              zoom={zoom}
              playing={pb.playing}
              audioKept={kept.system || kept.mic}
              onSeek={pb.seek}
              onScrub={(active) => active && pb.pause()}
              onInChange={setIn}
              onOutChange={setOut}
            />
          </div>
        </div>
      </div>

      <TrimExportBar
        mode={mode}
        onModeChange={setMode}
        phase={phase}
        outputName={copyName}
        canExport={trimmed || tracksChanged}
        onSaveCopy={() => startExport("copy")}
        onReplace={() => setConfirmReplace(true)}
        onCancel={onClose}
        onCancelExport={stopExport}
        onReveal={() => {
          // Prototype-local: the real app calls the shell to reveal the file in Finder.
        }}
        onDone={onClose}
      />

      {confirmReplace && (
        <ReplaceSheet
          fileName={originalName}
          fromDuration={formatShortClock(duration)}
          toDuration={formatShortClock(keep)}
          onCancel={() => setConfirmReplace(false)}
          onConfirm={() => startExport("replace")}
        />
      )}
    </div>
  );
}

function Readout({ label, value }: { label: string; value: string }) {
  return (
    <span className="flex items-baseline gap-1.5">
      <span className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">{label}</span>
      <span className="font-mono text-[12px] tabular">{value}</span>
    </span>
  );
}

function IconButton({ label, onClick, children }: { label: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="shrink-0 text-muted-foreground hover:text-foreground"
    >
      {children}
    </Button>
  );
}
