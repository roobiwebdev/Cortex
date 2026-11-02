import * as React from "react";

import { Badge } from "@starter/ui/components/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@starter/ui/components/select";
import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: SettingsPage, SettingsGroup, SettingsRow, settings row, hint, micro header
 * WHAT:  Layout building blocks for every Settings section: page title + subtitle,
 *        an uppercase micro-header over a card of rows, and a row (label, one-line hint, control).
 * WHY:   Every section must look the same and every row must say what the setting does,
 *        so the rhythm (spacing, dividers, hint style) lives in one place.
 * WHERE: apps/desktop/src/surfaces/settings/* sections.
 */
export function SettingsPage({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mx-auto w-full max-w-[580px] px-8 pb-10">
      <header className="mb-5">
        <h1 className="font-display text-[26px] leading-tight font-bold tracking-[-0.01em]">{title}</h1>
        <p className="mt-1 text-[13px] text-muted-foreground">{subtitle}</p>
      </header>
      <div className="space-y-6">{children}</div>
    </div>
  );
}

export function SettingsGroup({
  title,
  footer,
  action,
  children,
}: {
  title?: string;
  /** Muted note under the card. */
  footer?: React.ReactNode;
  /** Small control aligned right of the header (e.g. "Add server"). */
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section>
      {(title ?? action) && (
        <div className="mb-1.5 flex min-h-5 items-end justify-between px-1">
          {title && (
            <h2 className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">{title}</h2>
          )}
          {action}
        </div>
      )}
      <div className="divide-y overflow-hidden rounded-[14px] border bg-card shadow-ledge">{children}</div>
      {footer && <p className="mt-1.5 px-1 text-xs leading-relaxed text-muted-foreground">{footer}</p>}
    </section>
  );
}

export function SettingsRow({
  label,
  hint,
  badge,
  htmlFor,
  disabled,
  stacked,
  children,
}: {
  label: React.ReactNode;
  /** One line saying what changes when this is set. */
  hint?: React.ReactNode;
  badge?: "new" | "soon" | "prototype";
  htmlFor?: string;
  disabled?: boolean;
  /** Put the control under the label (wide controls like sliders and segmented groups). */
  stacked?: boolean;
  children?: React.ReactNode;
}) {
  const Label = htmlFor ? "label" : "div";
  return (
    <div
      className={cn(
        "flex min-h-[52px] gap-x-6 gap-y-2.5 px-4 py-3 transition-opacity",
        stacked ? "flex-col" : "items-center justify-between",
        disabled && "opacity-45",
      )}
    >
      <div className="min-w-0 flex-1">
        <Label htmlFor={htmlFor} className="flex items-center gap-2 text-[13px] leading-5 font-medium">
          {label}
          {badge && <RowBadge kind={badge} />}
        </Label>
        {hint && <p className="mt-0.5 text-xs leading-[1.45] text-muted-foreground">{hint}</p>}
      </div>
      {children && <div className={cn("flex shrink-0 items-center gap-2", stacked && "w-full")}>{children}</div>}
    </div>
  );
}

function RowBadge({ kind }: { kind: "new" | "soon" | "prototype" }) {
  const text = kind === "new" ? "New" : kind === "soon" ? "Coming soon" : "Prototype";
  return (
    <Badge
      className={cn(
        "h-[18px] rounded-[6px] px-1.5 text-[10px] font-semibold tracking-wide",
        kind === "new" ? "bg-accent text-accent-text" : "bg-muted text-muted-foreground",
      )}
    >
      {text}
    </Badge>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: ChoiceSelect, typed select, popup button, settings dropdown
 * WHAT:  A Radix Select bound to a typed string union, with optional per-option detail text.
 * WHY:   Settings values are unions (codec, resolution…); mapping back by lookup keeps them typed without casts.
 * WHERE: settings sections.
 */
export function ChoiceSelect<T extends string>({
  value,
  onChange,
  options,
  disabled,
  className,
  "aria-label": ariaLabel,
}: {
  value: T;
  onChange: (v: T) => void;
  options: ReadonlyArray<{ value: T; label: string; detail?: string; disabled?: boolean }>;
  disabled?: boolean;
  className?: string;
  "aria-label": string;
}) {
  return (
    <Select
      value={value}
      disabled={disabled}
      onValueChange={(v) => {
        const hit = options.find((o) => o.value === v);
        if (hit) onChange(hit.value);
      }}
    >
      <SelectTrigger
        aria-label={ariaLabel}
        size="sm"
        className={cn("min-w-36 rounded-[10px] bg-card text-[13px]", className)}
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent className="rounded-[10px]">
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value} disabled={o.disabled} className="rounded-[6px] text-[13px]">
            {o.label}
            {o.detail && <span className="text-xs text-muted-foreground">{o.detail}</span>}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: NumberField, unit field, ms field, dB field, numeric input
 * WHAT:  A compact numeric input with a trailing unit, committing on blur or Enter and clamping to a range.
 * WHY:   Offsets, bitrates and sizes are typed values; committing on blur avoids half-typed numbers
 *        reaching the engine.
 * WHERE: settings sections.
 */
export function NumberField({
  value,
  onCommit,
  unit,
  min,
  max,
  step = 1,
  disabled,
  width = "w-[92px]",
  "aria-label": ariaLabel,
}: {
  value: number;
  onCommit: (v: number) => void;
  unit?: string;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  width?: string;
  "aria-label": string;
}) {
  // null = not editing, so the field always shows the engine value until the user types.
  const [draft, setDraft] = React.useState<string | null>(null);

  function commit() {
    if (draft === null) return;
    const n = Number(draft.replace("−", "-"));
    setDraft(null);
    if (!draft.trim() || !Number.isFinite(n)) return;
    const clamped = Math.min(max, Math.max(min, Math.round(n / step) * step));
    if (clamped !== value) onCommit(clamped);
  }

  return (
    <div
      className={cn(
        "flex h-7 items-center rounded-[8px] border bg-card pr-2 transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/30",
        disabled && "opacity-50",
        width,
      )}
    >
      <input
        inputMode="decimal"
        aria-label={ariaLabel}
        disabled={disabled}
        value={draft ?? String(value)}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            const next = Math.min(max, Math.max(min, value + (e.key === "ArrowUp" ? step : -step)));
            onCommit(next);
          }
        }}
        className="tabular h-full w-full min-w-0 bg-transparent px-2 text-right text-[13px] outline-none disabled:cursor-not-allowed"
      />
      {unit && <span className="shrink-0 text-xs text-muted-foreground">{unit}</span>}
    </div>
  );
}

/** Right-aligned value readout next to a slider (tabular so it does not jitter). */
export function Readout({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <span className={cn("tabular w-16 shrink-0 text-right text-[13px] text-muted-foreground", className)}>
      {children}
    </span>
  );
}
