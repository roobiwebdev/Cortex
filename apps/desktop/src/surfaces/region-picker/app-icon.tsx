import { AppWindow, Compass, FileText, PenTool, SquareTerminal, type LucideIcon } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import { useAppIcon } from "../../lib/app-icons";

// Stand-ins while a real icon loads, and in the browser harness.
const ICONS: Record<string, LucideIcon> = {
  Ghostty: SquareTerminal,
  Figma: PenTool,
  Preview: FileText,
  Safari: Compass,
};

/**
 * SOURCE OF TRUTH KEYWORDS: AppIconTile, app icon, window picker icon, app picker, icon tile
 * WHAT:  An app's real icon (by bundle ID, from the engine), or a rounded-square tile with a
 *        stand-in glyph until it loads or when there is none.
 * WHY:   Window and app pickers identify sources by icon first, title second.
 * WHERE: region picker snap label, window grid, app list; the main window's App list.
 */
export function AppIconTile({
  app,
  bundleId,
  size = 22,
  className,
}: {
  app: string;
  bundleId?: string;
  size?: number;
  className?: string;
}) {
  const src = useAppIcon(bundleId);
  if (src) {
    return (
      <span
        aria-hidden
        className={cn("shrink-0 bg-contain bg-center bg-no-repeat", className, "shadow-none")}
        style={{ width: size, height: size, backgroundImage: `url("${src}")` }}
      />
    );
  }
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
