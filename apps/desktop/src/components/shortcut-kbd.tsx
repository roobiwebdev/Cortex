import { Kbd } from "@starter/ui/components/kbd";
import { cn } from "@starter/ui/lib/utils";

import { usePlatform } from "../lib/platform";
import { formatShortcut, shortcutKeys, shortcutSpoken, type ShortcutAction } from "../lib/shortcuts";

/** The shortcut's written form on the running platform, for tooltips: "Start recording (⇧⌘R)". */
export function useShortcut(action: ShortcutAction): string {
  return formatShortcut(action, usePlatform());
}

/**
 * SOURCE OF TRUTH KEYWORDS: ShortcutKbd, shortcut key caps, Kbd, hotkey label, per-platform keys
 * WHAT:  A shortcut drawn as key caps for the running platform: one cap reading "⇧⌘R" on
 *        macOS, separate "Ctrl" "Shift" "R" caps on Windows and Linux.
 * WHY:   Mac glyphs read as one chord; spelled-out modifiers need a cap each to stay legible.
 *        Every surface draws shortcuts through this, so none can drift from `lib/shortcuts`.
 * WHERE: record bar, menu bar panel, settings (Live & Replay), sources sidebar hints.
 */
export function ShortcutKbd({ action, className }: { action: ShortcutAction; className?: string }) {
  const platform = usePlatform();
  if (platform === "macos") return <Kbd className={className}>{formatShortcut(action, platform)}</Kbd>;
  return (
    <span className="inline-flex items-center gap-0.5" aria-label={shortcutSpoken(action, platform)}>
      {shortcutKeys(action, platform).map((k) => (
        <Kbd key={k} className={cn(className)} aria-hidden>
          {k}
        </Kbd>
      ))}
    </span>
  );
}
