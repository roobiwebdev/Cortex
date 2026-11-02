import type * as React from "react";
import { Check, FolderOpen } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";

/** Fast = passthrough (NEW-TR-04), Exact = re-encode (NEW-TR-05). */
export type ExportMode = "fast" | "exact";
export type ExportOutput = "copy" | "replace";

export type ExportPhase =
  | { kind: "idle" }
  | { kind: "exporting"; output: ExportOutput; progress: number; secsLeft: number }
  | { kind: "done"; output: ExportOutput; fileName: string; detail: string };

const MODE_OPTIONS = [
  { value: "fast", label: "Fast (no re-encode)", title: "Copies the original frames. Takes seconds." },
  {
    value: "exact",
    label: "Exact (re-encode)",
    title: "Re-encodes the cut so every player starts on the exact frame.",
  },
] as const;

/**
 * SOURCE OF TRUTH KEYWORDS: TrimExportBar, export mode, save as copy, replace original, export progress
 * WHAT:  The trim window's bottom bar: export mode, output choice, then progress and the saved state.
 * WHY:   Export is one decision (how precise) and one action (where it goes); the same bar then
 *        shows progress and the result so the user never loses context.
 * WHERE: TrimWindow.
 */
export function TrimExportBar({
  mode,
  onModeChange,
  phase,
  outputName,
  canExport,
  onSaveCopy,
  onReplace,
  onCancel,
  onCancelExport,
  onReveal,
  onDone,
}: {
  mode: ExportMode;
  onModeChange: (m: ExportMode) => void;
  phase: ExportPhase;
  outputName: string;
  canExport: boolean;
  onSaveCopy: () => void;
  onReplace: () => void;
  onCancel: () => void;
  onCancelExport: () => void;
  onReveal: () => void;
  onDone: () => void;
}) {
  if (phase.kind === "exporting") {
    const pct = Math.round(phase.progress * 100);
    return (
      <Bar>
        <div className="min-w-0 flex-1" aria-live="polite">
          <div className="flex items-baseline justify-between gap-4">
            <p className="truncate text-[13px] font-medium">
              {phase.output === "copy" ? "Saving" : "Replacing with"} “{outputName}”
            </p>
            <p className="shrink-0 font-mono text-[12px] text-muted-foreground tabular">
              {pct}% · {phase.secsLeft > 0 ? `about ${phase.secsLeft} s left` : "finishing"}
            </p>
          </div>
          <div
            role="progressbar"
            aria-label="Export progress"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct}
            className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted"
          >
            <div className="h-full rounded-full bg-primary" style={{ width: `${pct}%` }} />
          </div>
          <p className="mt-1.5 text-[11.5px] text-muted-foreground">
            {mode === "fast" ? "Copying frames without re-encoding" : "Re-encoding the cut, frame-accurate"}
          </p>
        </div>
        <Button variant="soft" size="lg" onClick={onCancelExport}>
          Stop export
        </Button>
      </Bar>
    );
  }

  if (phase.kind === "done") {
    return (
      <Bar>
        <div
          className="flex min-w-0 flex-1 animate-in items-center gap-3 duration-200 ease-out-quick fade-in-0"
          aria-live="polite"
        >
          <span className="grid size-9 shrink-0 place-items-center rounded-full bg-ok-tint text-ok">
            <Check className="size-4.5" strokeWidth={2.5} />
          </span>
          <div className="min-w-0">
            <p className="truncate text-[13px] font-semibold">
              {phase.output === "copy" ? "Saved" : "Original replaced"} · {phase.fileName}
            </p>
            <p className="truncate text-[12px] text-muted-foreground tabular">{phase.detail}</p>
          </div>
        </div>
        <Button variant="soft" size="lg" onClick={onReveal}>
          <FolderOpen />
          Reveal in Finder
        </Button>
        <Button variant="tactile" size="lg" className="px-5" onClick={onDone}>
          Done
        </Button>
      </Bar>
    );
  }

  return (
    <Bar>
      <div className="min-w-0 flex-1">
        <Segmented aria-label="Export mode" value={mode} onValueChange={onModeChange} options={MODE_OPTIONS} />
        <p className="mt-1.5 max-w-[520px] text-[11.5px] leading-snug text-muted-foreground">
          Fast cuts at the nearest keyframe in some players; Exact is frame-accurate but slower.
        </p>
      </div>
      <Button variant="ghost" size="lg" onClick={onCancel}>
        Cancel
      </Button>
      <Button variant="soft" size="lg" disabled={!canExport} onClick={onReplace}>
        Replace original
      </Button>
      <Button
        variant="tactile"
        size="lg"
        className="px-4"
        disabled={!canExport}
        onClick={onSaveCopy}
        title={`Saves “${outputName}” next to the original`}
      >
        Save as copy
      </Button>
    </Bar>
  );
}

function Bar({ children }: { children: React.ReactNode }) {
  return <div className="flex h-[76px] shrink-0 items-center gap-2 border-t bg-card px-4">{children}</div>;
}
