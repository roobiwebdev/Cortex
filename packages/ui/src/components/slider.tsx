"use client";

import * as React from "react";
import { Slider as SliderPrimitive } from "radix-ui";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: Slider, range, fader, gain, size slider, radix slider
 * WHAT:  A single-thumb or range slider on Radix, styled with theme tokens.
 * WHY:   Faders, overlay size/roundness and trim ranges all need one accessible slider.
 * WHERE: desktop mixer, camera overlay controls, settings, trim window.
 */
function Slider({ className, ...props }: React.ComponentProps<typeof SliderPrimitive.Root>) {
  const count = (props.value ?? props.defaultValue ?? [0]).length;
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn(
        "relative flex w-full touch-none items-center select-none data-disabled:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col",
        className,
      )}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-1 w-full grow overflow-hidden rounded-full bg-foreground/10 data-[orientation=vertical]:h-full data-[orientation=vertical]:w-1">
        <SliderPrimitive.Range className="absolute h-full bg-primary data-[orientation=vertical]:w-full" />
      </SliderPrimitive.Track>
      {Array.from({ length: count }, (_, i) => (
        <SliderPrimitive.Thumb
          key={i}
          className="block size-3.5 rounded-full border border-border bg-card shadow-ledge transition-[box-shadow] outline-none hover:ring-4 hover:ring-ring/15 focus-visible:ring-4 focus-visible:ring-ring/30"
        />
      ))}
    </SliderPrimitive.Root>
  );
}

export { Slider };
