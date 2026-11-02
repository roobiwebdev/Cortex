import { AppWindow, Compass, FileText, PenTool, SquareTerminal, type LucideIcon } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

// Prototype stand-ins for real app icons (the engine will send NSWorkspace icons as small PNGs).
const ICONS: Record<string, LucideIcon> = {
  Ghostty: SquareTerminal,
  Figma: PenTool,
  Preview: FileText,
  Safari: Compass,
};

/**
 * SOURCE OF TRUTH KEYWORDS: AppIconTile, app icon, window picker icon, app picker, icon tile
 * WHAT:  A rounded-square icon tile standing in for an app's icon.
 * WHY:   Window and app pickers identify sources by icon first, title second.
 * WHERE: region picker snap label, window grid, app list.
 */
export function AppIconTile({ app, size = 22, className }: { app: string; size?: number; className?: string }) {
  const Icon = ICONS[app] ?? AppWindow;
  return (
    <span
      aria-hidden
      className={cn(
        "grid shrink-0 place-items-center rounded-[28%] bg-accent-tint text-accent-text shadow-ledge",
        className,
      )}
      style={{ width: size, height: size }}
    >
      <Icon style={{ width: size * 0.58, height: size * 0.58 }} strokeWidth={2} />
    </span>
  );
}
