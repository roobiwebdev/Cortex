import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: BrandMark, Cortex logo, app mark, C ring, record dot, tray glyph, branding
 * WHAT:  The Cortex mark as an inline SVG: a heavy "C" ring around the record dot. The ring
 *        takes `currentColor` (text-primary by default) and the dot takes the rec token.
 *        `mono` draws the dot in `currentColor` too, like the menu-bar template icon.
 * WHY:   Inline SVG follows the theme tokens in light and dark without a raster per theme.
 *        The geometry matches apps/desktop/branding/mark.svg, the master for every icon.
 * WHERE: About, onboarding welcome, menu-bar panel header, prototype menu-bar status item.
 */
export function BrandMark({ className, mono = false, title }: { className?: string; mono?: boolean; title?: string }) {
  return (
    <svg
      viewBox="212 212 600 600"
      className={cn("shrink-0 text-primary", className)}
      role={title ? "img" : undefined}
      aria-hidden={title ? undefined : true}
    >
      {title && <title>{title}</title>}
      <path
        d="M664.6 342.6A228 228 0 1 0 664.6 681.4"
        fill="none"
        stroke="currentColor"
        strokeWidth={128}
        strokeLinecap="round"
      />
      <circle cx="512" cy="512" r="92" className={mono ? "fill-current" : "fill-rec"} />
    </svg>
  );
}
