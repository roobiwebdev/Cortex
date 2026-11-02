import * as React from "react";
import { ChevronDown } from "lucide-react";

import { Switch } from "@starter/ui/components/switch";
import { Tooltip, TooltipContent, TooltipTrigger } from "@starter/ui/components/tooltip";
import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: SidebarSection, collapsible section, micro-label, SwitchRow, FieldRow, Tip, main window parts
 * WHAT:  Small layout pieces shared by the main window: collapsible sidebar section with an
 *        uppercase micro-label, a labelled switch row with a hint, a labelled field row and a
 *        tooltip shorthand.
 * WHY:   Every sidebar group and option row must look and behave the same (spacing, hint
 *        tone, keyboard focus), so they are built once here.
 * WHERE: main window sidebar, camera section, options section, title bar, record bar.
 */
export function SidebarSection({
  title,
  aside,
  defaultOpen = true,
  children,
}: {
  title: string;
  /** Small trailing text next to the label, e.g. "Locked while recording". */
  aside?: React.ReactNode;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = React.useState(defaultOpen);
  const id = React.useId();
  return (
    <section className="border-b border-sidebar-border px-4 py-3 last:border-b-0">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((v) => !v)}
        className="group flex w-full items-center gap-1.5 rounded-md py-1 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase group-hover:text-foreground">
          {title}
        </span>
        {aside && <span className="truncate text-[11px] text-muted-foreground/80">{aside}</span>}
        <ChevronDown
          aria-hidden
          className={cn(
            "ml-auto size-3.5 text-muted-foreground transition-transform duration-200 ease-out-quick",
            !open && "-rotate-90",
          )}
        />
      </button>
      {open && (
        <div id={id} className="mt-2.5 space-y-3">
          {children}
        </div>
      )}
    </section>
  );
}

export function SwitchRow({
  label,
  hint,
  checked,
  onCheckedChange,
  disabled,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  const id = React.useId();
  return (
    <div className={cn("flex items-start gap-3", disabled && "opacity-50")}>
      <label htmlFor={id} className="min-w-0 flex-1 cursor-pointer">
        <span className="block text-[13px] leading-5">{label}</span>
        {hint && <span className="block text-[11.5px] leading-4 text-muted-foreground">{hint}</span>}
      </label>
      <Switch id={id} className="mt-0.5" checked={checked} onCheckedChange={onCheckedChange} disabled={disabled} />
    </div>
  );
}

export function FieldRow({
  label,
  value,
  children,
}: {
  label: string;
  /** Right-aligned readout, e.g. "22%". */
  value?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between text-[12px]">
        <span className="text-muted-foreground">{label}</span>
        {value !== undefined && <span className="tabular font-mono text-[11px] text-foreground">{value}</span>}
      </div>
      {children}
    </div>
  );
}

export function Tip({
  label,
  children,
  side = "top",
}: {
  label: React.ReactNode;
  children: React.ReactElement;
  side?: "top" | "bottom";
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side={side} sideOffset={6}>
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

/** Sliders on the sunken sidebar need a darker track than the default `bg-muted`. */
export const SIDEBAR_SLIDER = "[&>span:first-child]:bg-foreground/10";

/** Uppercase micro-label used inside a section to group a few rows. */
export function MicroLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="pt-1 text-[10.5px] font-medium tracking-[0.06em] text-muted-foreground/80 uppercase">
      {children}
    </div>
  );
}
