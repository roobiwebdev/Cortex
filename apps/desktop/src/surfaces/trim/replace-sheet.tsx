import * as React from "react";

import { Button } from "@starter/ui/components/button";

/**
 * SOURCE OF TRUTH KEYWORDS: ReplaceSheet, confirm replace original, window sheet, destructive confirm
 * WHAT:  A macOS-style sheet that drops from the title bar to confirm replacing the original file.
 * WHY:   Replacing is the only trim action that touches the source; it needs one deliberate
 *        extra step and must say exactly what happens to the old file.
 * WHERE: TrimWindow, when "Replace original" is pressed.
 */
export function ReplaceSheet({
  fileName,
  fromDuration,
  toDuration,
  onCancel,
  onConfirm,
}: {
  fileName: string;
  fromDuration: string;
  toDuration: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const cancelRef = React.useRef<HTMLButtonElement>(null);
  React.useEffect(() => cancelRef.current?.focus(), []);

  return (
    <div
      className="absolute inset-x-0 top-11 bottom-0 z-40 flex justify-center bg-background/40"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onCancel();
        }
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="replace-title"
        aria-describedby="replace-desc"
        className="h-fit w-[400px] animate-in rounded-b-[18px] border-x border-b bg-card p-5 shadow-float duration-200 ease-out-quick slide-in-from-top-4 fade-in-0"
      >
        <h2 id="replace-title" className="text-[15px] font-semibold">
          Replace the original recording?
        </h2>
        <p id="replace-desc" className="mt-1.5 text-[13px] leading-relaxed text-muted-foreground">
          “{fileName}” becomes the trimmed {toDuration} version. The original {fromDuration} file moves to the Trash, so
          you can still restore it from there.
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button ref={cancelRef} variant="soft" size="lg" onClick={onCancel}>
            Keep original
          </Button>
          <Button variant="tactile" size="lg" onClick={onConfirm}>
            Replace original
          </Button>
        </div>
      </div>
    </div>
  );
}
