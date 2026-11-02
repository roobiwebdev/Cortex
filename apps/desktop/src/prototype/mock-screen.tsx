import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: MockScreen, fake desktop, preview placeholder, design prototype
 * WHAT:  A stylised fake desktop that stands in for the live screen during design review.
 * WHY:   The real preview is a native Metal view; the prototype needs believable content.
 * WHERE: PreviewStage slot, region picker backdrop, trim window frames (prototype only).
 */
export function MockScreen({ variant = "desktop", className }: { variant?: "desktop" | "window"; className?: string }) {
  return (
    <div
      className={cn(
        "relative size-full overflow-hidden bg-gradient-to-br from-accent-solid/70 via-accent-tint to-muted",
        className,
      )}
    >
      {variant === "desktop" && <div className="absolute inset-x-0 top-0 h-[3.2%] bg-card/70 backdrop-blur" />}
      <div
        className={cn(
          "absolute flex flex-col overflow-hidden rounded-[1.2%] bg-card shadow-float",
          variant === "desktop" ? "top-[11%] left-[9%] h-[74%] w-[62%]" : "inset-[3%]",
        )}
      >
        <div className="flex h-[7%] min-h-2 items-center gap-[1.2%] border-b px-[2%]">
          <span className="aspect-square h-[38%] rounded-full bg-rec/70" />
          <span className="aspect-square h-[38%] rounded-full bg-warn/70" />
          <span className="aspect-square h-[38%] rounded-full bg-ok/70" />
        </div>
        <div className="flex flex-1">
          <div className="w-[22%] space-y-[6%] bg-muted p-[3%]">
            {[70, 55, 80, 45, 60].map((w, i) => (
              <div key={i} className="h-[3%] min-h-1 rounded-full bg-muted-foreground/25" style={{ width: `${w}%` }} />
            ))}
          </div>
          <div className="flex-1 space-y-[2.6%] p-[4%]">
            <div className="h-[6%] min-h-1.5 w-[48%] rounded-full bg-foreground/70" />
            {[92, 84, 88, 60, 0, 90, 76, 82, 40].map((w, i) =>
              w === 0 ? (
                <div key={i} className="h-[4%]" />
              ) : (
                <div
                  key={i}
                  className="h-[2.6%] min-h-1 rounded-full bg-muted-foreground/30"
                  style={{ width: `${w}%` }}
                />
              ),
            )}
            <div className="mt-[4%] grid h-[24%] grid-cols-3 gap-[3%]">
              <div className="rounded-[6%] bg-accent-tint" />
              <div className="rounded-[6%] bg-muted" />
              <div className="rounded-[6%] bg-muted" />
            </div>
          </div>
        </div>
      </div>
      {variant === "desktop" && (
        <div className="absolute top-[22%] right-[6%] h-[34%] w-[20%] space-y-[8%] rounded-[4%] bg-card/80 p-[1.6%] shadow-float backdrop-blur">
          <div className="h-[8%] w-[60%] rounded-full bg-foreground/60" />
          <div className="h-[46%] rounded-[6%] bg-accent-tint" />
          <div className="h-[6%] w-[80%] rounded-full bg-muted-foreground/30" />
          <div className="h-[6%] w-[50%] rounded-full bg-muted-foreground/30" />
        </div>
      )}
    </div>
  );
}
