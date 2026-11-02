import * as React from "react";
import { Check, Info, Monitor, RotateCw } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Checkbox } from "@starter/ui/components/checkbox";
import { Input } from "@starter/ui/components/input";
import { Segmented } from "@starter/ui/components/segmented";
import { cn } from "@starter/ui/lib/utils";

import type { CaptureApp, CaptureWindow, Display } from "../../lib/types";
import { MockScreen } from "../../prototype/mock-screen";
import { AppIconTile } from "./app-icon";
import { ASPECTS, MIN_SIZE, type Aspect, type Size } from "./geometry";

/**
 * SOURCE OF TRUTH KEYWORDS: AreaOptions, aspect presets, numeric size, W H inputs, remember last area
 * WHAT:  Area-mode options: aspect lock, exact width/height in display pixels, remember toggle.
 * WHY:   Pros need exact output sizes (1920×1080, 1080×1920) without pixel-hunting by hand.
 * WHERE: RegionToolbar options tier, Area mode.
 */
export function AreaOptions({
  aspect,
  onAspect,
  size,
  onSize,
  remember,
  onRemember,
}: {
  aspect: Aspect;
  onAspect: (a: Aspect) => void;
  size: Size | null;
  onSize: (which: "w" | "h", value: number) => void;
  remember: boolean;
  onRemember: (v: boolean) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
      <div className="flex items-center gap-2">
        <span className="text-[12px] text-muted-foreground">Aspect</span>
        <Segmented<Aspect>
          aria-label="Aspect ratio"
          size="sm"
          value={aspect}
          onValueChange={onAspect}
          options={ASPECTS.map((a) => ({ value: a.value, label: <span className="tabular">{a.label}</span> }))}
        />
      </div>
      <div className="flex items-center gap-1.5">
        <NumberField label="W" name="Width in pixels" value={size?.w ?? null} onCommit={(n) => onSize("w", n)} />
        <span aria-hidden className="text-[12px] text-muted-foreground">
          ×
        </span>
        <NumberField label="H" name="Height in pixels" value={size?.h ?? null} onCommit={(n) => onSize("h", n)} />
        <span className="text-[12px] text-muted-foreground">px</span>
      </div>
      <label className="ml-auto flex cursor-pointer items-center gap-2 text-[12.5px]">
        <Checkbox checked={remember} onCheckedChange={(v) => onRemember(v)} className="rounded-[5px]" />
        Remember last area
      </label>
    </div>
  );
}

function NumberField({
  label,
  name,
  value,
  onCommit,
}: {
  label: string;
  name: string;
  value: number | null;
  onCommit: (n: number) => void;
}) {
  // Draft text only while focused; otherwise the field mirrors the live selection.
  const [draft, setDraft] = React.useState<string | null>(null);
  const commit = () => {
    const n = Number.parseInt(draft ?? "", 10);
    if (Number.isFinite(n) && n >= MIN_SIZE) onCommit(n);
    setDraft(null);
  };
  return (
    <div className="flex items-center gap-1">
      <span aria-hidden className="text-[11px] font-medium text-muted-foreground">
        {label}
      </span>
      <Input
        aria-label={name}
        inputMode="numeric"
        disabled={value === null}
        placeholder="—"
        value={draft ?? (value === null ? "" : String(value))}
        onFocus={(e) => {
          setDraft(String(value ?? ""));
          e.currentTarget.select();
        }}
        onChange={(e) => setDraft(e.target.value.replace(/[^0-9]/g, ""))}
        onBlur={commit}
        onKeyDown={(e) => {
          // Keys stay inside the field: Esc reverts instead of closing the picker.
          e.stopPropagation();
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") {
            setDraft(null);
            e.currentTarget.blur();
          }
        }}
        className="tabular h-7 w-[66px] rounded-[8px] bg-background text-right text-[12.5px]"
      />
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: WindowGrid, window picker, window thumbnails, pick window, refresh windows
 * WHAT:  A visual grid of capturable windows (thumbnail, app icon, title) with a refresh button.
 * WHY:   Windows on other Spaces or behind others can't be clicked on the desktop (NEW-WP-01, CAP-12).
 * WHERE: RegionToolbar options tier, Window mode.
 */
export function WindowGrid({
  windows,
  selectedId,
  onSelect,
  onRefresh,
}: {
  windows: readonly CaptureWindow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onRefresh: () => void;
}) {
  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between px-1">
        <span className="text-[12px] text-muted-foreground">
          {windows.length} windows · click one here or on the screen
        </span>
        <Button variant="ghost" size="icon-xs" aria-label="Refresh window list" title="Refresh" onClick={onRefresh}>
          <RotateCw />
        </Button>
      </div>
      <div role="radiogroup" aria-label="Windows" className="grid grid-cols-4 gap-1.5">
        {windows.map((w) => {
          const selected = w.id === selectedId;
          return (
            <button
              key={w.id}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() => onSelect(w.id)}
              className={cn(
                "group rounded-[12px] p-1.5 text-left transition-colors outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50",
                selected && "bg-accent hover:bg-accent",
              )}
            >
              <div
                className={cn(
                  "relative aspect-[16/10] overflow-hidden rounded-[8px] border transition-shadow",
                  selected && "border-transparent ring-2 ring-accent-solid",
                )}
              >
                <MockScreen variant="window" />
              </div>
              <div className="mt-1.5 flex items-center gap-1.5">
                <AppIconTile app={w.app} size={18} className="shadow-none" />
                <div className="min-w-0 leading-tight">
                  <div className={cn("truncate text-[12px] font-medium", selected && "text-accent-foreground")}>
                    {w.app}
                  </div>
                  <div className="truncate text-[11px] text-muted-foreground">{w.title}</div>
                </div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: AppList, app picker, capture app, all windows of app, bundle id
 * WHAT:  Apps with their window counts; choosing one records every window it has or opens.
 * WHY:   NEW-AP-01/02: following an app is what people mean by "record Figma".
 * WHERE: RegionToolbar options tier, App mode.
 */
export function AppList({
  apps,
  selectedId,
  onSelect,
}: {
  apps: readonly CaptureApp[];
  selectedId: string | null;
  onSelect: (bundleId: string) => void;
}) {
  const selected = apps.find((a) => a.bundleId === selectedId);
  return (
    <div>
      <div role="radiogroup" aria-label="Apps" className="grid grid-cols-3 gap-1.5">
        {apps.map((a) => (
          <OptionRow
            key={a.bundleId}
            selected={a.bundleId === selectedId}
            onSelect={() => onSelect(a.bundleId)}
            icon={<AppIconTile app={a.name} size={28} />}
            title={a.name}
            detail={`${a.windows} ${a.windows === 1 ? "window" : "windows"}`}
          />
        ))}
      </div>
      <Note>
        {selected
          ? `Records every ${selected.name} window, even new ones. Other apps stay out.`
          : "Pick an app. Windows it opens while recording are included."}
      </Note>
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: DisplayList, full screen capture, display picker, Display N W×H
 * WHAT:  Connected displays with native pixel sizes; the overlay's own display is marked.
 * WHY:   Full screen mode needs to say exactly which display and resolution will be recorded (CAP-02).
 * WHERE: RegionToolbar options tier, Full screen mode.
 */
export function DisplayList({
  displays,
  selectedId,
  currentId,
  onSelect,
}: {
  displays: readonly Display[];
  selectedId: number;
  currentId: number;
  onSelect: (id: number) => void;
}) {
  return (
    <div>
      <div role="radiogroup" aria-label="Displays" className="grid grid-cols-2 gap-1.5">
        {displays.map((d, i) => (
          <OptionRow
            key={d.id}
            selected={d.id === selectedId}
            onSelect={() => onSelect(d.id)}
            icon={
              <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-muted text-muted-foreground shadow-ledge">
                <Monitor className="size-4" />
              </span>
            }
            title={`Display ${i + 1} — ${d.name}`}
            detail={`${d.width} × ${d.height}${d.id === currentId ? " · this display" : ""}`}
          />
        ))}
      </div>
      <Note>The whole display is recorded, including the menu bar and Dock. Cortex's own windows never appear.</Note>
    </div>
  );
}

function OptionRow({
  selected,
  onSelect,
  icon,
  title,
  detail,
}: {
  selected: boolean;
  onSelect: () => void;
  icon: React.ReactNode;
  title: string;
  detail: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      className={cn(
        "flex items-center gap-2.5 rounded-[12px] border p-2 text-left transition-colors outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50",
        selected && "border-accent-solid/50 bg-accent hover:bg-accent",
      )}
    >
      {icon}
      <div className="min-w-0 flex-1 leading-tight">
        <div className={cn("truncate text-[13px] font-medium", selected && "text-accent-foreground")}>{title}</div>
        <div className="tabular truncate text-[11.5px] text-muted-foreground">{detail}</div>
      </div>
      <span
        aria-hidden
        className={cn(
          "grid size-4 shrink-0 place-items-center rounded-full border transition-colors",
          selected && "border-primary bg-primary text-primary-foreground",
        )}
      >
        {selected && <Check className="size-2.5" strokeWidth={3.5} />}
      </span>
    </button>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-2 flex items-center gap-1.5 px-1 text-[11.5px] text-muted-foreground">
      <Info className="size-3.5 shrink-0" />
      {children}
    </p>
  );
}
