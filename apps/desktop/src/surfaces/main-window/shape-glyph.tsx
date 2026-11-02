import type { PipShape } from "../../lib/types";

/**
 * SOURCE OF TRUTH KEYWORDS: ShapeGlyph, PIP_SHAPES, overlay shape icon, squircle glyph, camera shape picker
 * WHAT:  A 20×20 line glyph for each camera overlay shape, plus the ordered shape list with labels.
 * WHY:   lucide has no icons for squircle/pill overlay shapes; drawing them keeps the picker
 *        literal (the glyph is the shape you get).
 * WHERE: main window camera section shape picker.
 */
export const PIP_SHAPES: ReadonlyArray<{ value: PipShape; label: string }> = [
  { value: "rounded", label: "Rounded" },
  { value: "circle", label: "Circle" },
  { value: "square", label: "Square" },
  { value: "pill", label: "Pill" },
  { value: "squircle", label: "Squircle" },
  { value: "rect", label: "Rectangle" },
];

export function ShapeGlyph({ shape }: { shape: PipShape }) {
  const common = { fill: "none", stroke: "currentColor", strokeWidth: 1.6 } as const;
  return (
    <svg viewBox="0 0 20 20" className="size-5" aria-hidden>
      {shape === "rounded" && <rect x="2" y="5" width="16" height="10" rx="3" {...common} />}
      {shape === "circle" && <circle cx="10" cy="10" r="7" {...common} />}
      {shape === "square" && <rect x="3.5" y="3.5" width="13" height="13" rx="1" {...common} />}
      {shape === "pill" && <rect x="2" y="5.5" width="16" height="9" rx="4.5" {...common} />}
      {shape === "squircle" && <path d="M10 3c5.6 0 7 1.4 7 7s-1.4 7-7 7-7-1.4-7-7 1.4-7 7-7Z" {...common} />}
      {shape === "rect" && <rect x="2" y="5" width="16" height="10" rx="0.5" {...common} />}
    </svg>
  );
}
