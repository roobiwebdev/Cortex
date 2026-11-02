import * as React from "react";
import { Check, Monitor, SquareDashed } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";
import { cn } from "@starter/ui/lib/utils";

import { engine } from "../../lib/mock-engine";
import type { CaptureTarget, EngineState } from "../../lib/types";
import { MockScreen } from "../../prototype/mock-screen";
import type { SurfaceId } from "../../prototype/surfaces";

type CaptureTab = "display" | "window" | "app" | "area";

function set(target: CaptureTarget) {
  engine.update({ target });
}

function tabFor(target: CaptureTarget): CaptureTab {
  return target.kind === "region" ? "area" : target.kind;
}

/**
 * SOURCE OF TRUTH KEYWORDS: CaptureSection, capture target picker, display list, window list, app capture, area
 * WHAT:  The sidebar's capture picker: Display | Window | App | Area, each with its own list,
 *        writing the chosen source to `settings.target`.
 * WHY:   One place to choose what is recorded (CAP-02, CAP-06, CAP-07); the app and area
 *        kinds are new in Cortex. Lists are cards so long names truncate with a hover title.
 * WHERE: main window sidebar, CAPTURE section.
 */
export function CaptureSection({
  e,
  locked,
  navigate,
}: {
  e: EngineState;
  /** Sources cannot change mid-recording (UI-15). */
  locked: boolean;
  navigate: (id: SurfaceId) => void;
}) {
  const target = e.settings.target;
  const [tab, setTab] = React.useState<CaptureTab>(() => tabFor(target));

  return (
    <div className={cn("space-y-2.5", locked && "pointer-events-none opacity-55")} aria-disabled={locked || undefined}>
      <Segmented<CaptureTab>
        aria-label="What to capture"
        size="sm"
        className="w-full bg-foreground/[0.06]"
        value={tab}
        onValueChange={setTab}
        options={[
          { value: "display", label: "Display" },
          { value: "window", label: "Window" },
          { value: "app", label: "App" },
          { value: "area", label: "Area" },
        ]}
      />

      {tab === "display" && (
        <SourceList label="Displays">
          {e.displays.map((d, i) => (
            <SourceCard
              key={d.id}
              selected={target.kind === "display" && target.displayId === d.id}
              onSelect={() => set({ kind: "display", displayId: d.id })}
              leading={
                <span className="grid size-8 place-items-center rounded-[8px] bg-accent text-accent-foreground">
                  <Monitor className="size-4" />
                </span>
              }
              title={d.name}
              detail={`Display ${i + 1} · ${d.width} × ${d.height}`}
            />
          ))}
        </SourceList>
      )}

      {tab === "window" && (
        <SourceList label="Open windows">
          {e.windows.map((w) => (
            <SourceCard
              key={w.id}
              selected={target.kind === "window" && target.windowId === w.id}
              onSelect={() => set({ kind: "window", windowId: w.id })}
              leading={
                <span className="block h-8 w-12 overflow-hidden rounded-[6px] ring-1 ring-foreground/10">
                  <MockScreen variant="window" />
                </span>
              }
              title={w.app}
              detail={w.title}
            />
          ))}
        </SourceList>
      )}

      {tab === "app" && (
        <SourceList label="Apps">
          {e.apps.map((a) => (
            <SourceCard
              key={a.bundleId}
              selected={target.kind === "app" && target.bundleId === a.bundleId}
              onSelect={() => set({ kind: "app", bundleId: a.bundleId })}
              leading={
                <span className="grid size-8 place-items-center rounded-[8px] bg-card text-[13px] font-semibold shadow-ledge text-foreground">
                  {a.name.slice(0, 1)}
                </span>
              }
              title={a.name}
              detail={`All ${a.windows === 1 ? "1 window" : `${a.windows} windows`}, including new ones`}
            />
          ))}
        </SourceList>
      )}

      {tab === "area" && <AreaPanel e={e} navigate={navigate} />}
    </div>
  );
}

function AreaPanel({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const t = e.settings.target;
  const display = t.kind === "region" ? e.displays.find((d) => d.id === t.displayId) : undefined;
  return (
    <div className="space-y-2.5 rounded-[10px] border bg-card p-3">
      <div className="flex items-start gap-2.5">
        <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-muted text-muted-foreground">
          <SquareDashed className="size-4" />
        </span>
        {t.kind === "region" ? (
          <div className="min-w-0">
            <div className="tabular font-mono text-[12.5px]">
              {t.w} × {t.h}
            </div>
            <div className="truncate text-[11.5px] text-muted-foreground">
              at {t.x}, {t.y} on {display?.name ?? "a disconnected display"}
            </div>
          </div>
        ) : (
          <div className="text-[12px] leading-4 text-muted-foreground">
            Record part of a display. Drag a rectangle, or pick a window to snap to it.
          </div>
        )}
      </div>
      <Button variant="soft" size="sm" className="w-full" onClick={() => navigate("region")}>
        {t.kind === "region" ? "Change area…" : "Select area…"}
      </Button>
    </div>
  );
}

function SourceList({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div role="radiogroup" aria-label={label} className="space-y-1.5">
      {children}
    </div>
  );
}

function SourceCard({
  selected,
  onSelect,
  leading,
  title,
  detail,
}: {
  selected: boolean;
  onSelect: () => void;
  leading: React.ReactNode;
  title: string;
  detail: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      title={`${title} — ${detail}`}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-[10px] border p-2 text-left transition-[background,border-color,box-shadow] duration-150 outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
        selected ? "border-primary/50 bg-card shadow-ledge" : "border-transparent hover:bg-card/70",
      )}
    >
      <span className="shrink-0">{leading}</span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] leading-5 font-medium">{title}</span>
        <span className="tabular block truncate text-[11.5px] leading-4 text-muted-foreground">{detail}</span>
      </span>
      <Check className={cn("size-4 shrink-0 text-primary", !selected && "invisible")} aria-hidden />
    </button>
  );
}
