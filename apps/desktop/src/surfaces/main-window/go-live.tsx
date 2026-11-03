import * as React from "react";
import { KeyRound, Radio } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { CoachMark } from "../../components/coach-mark";
import { engine } from "../../lib/engine";
import { needsKey } from "../../lib/streaming";
import type { EngineState, StreamDest, StreamService } from "../../lib/types";
import type { SurfaceId } from "../../prototype/surfaces";

const SERVICE_NAME: Record<StreamService, string> = {
  youtube: "YouTube",
  twitch: "Twitch",
  kick: "Kick",
  custom: "Custom server",
};

/** Why Go live is disabled, in plain words (STR-05), or null when it can start. */
function blocker(dests: StreamDest[]): string | null {
  const on = dests.filter((d) => d.enabled);
  if (on.length === 0) return "Turn on a destination";
  const missingUrl = on.find((d) => d.service === "custom" && !d.url);
  if (missingUrl) return "Enter the server's URL in Settings";
  // RTMP(S) needs a key; a WHIP server may take none.
  const missingKey = on.find(needsKey);
  if (missingKey) return `Save the stream key for ${SERVICE_NAME[missingKey.service]} in Settings`;
  return null;
}

/**
 * SOURCE OF TRUTH KEYWORDS: GoLiveButton, streaming, destinations, stream key, live badge, popover
 * WHAT:  The record bar's "Go live" button with a popover listing stream destinations
 *        (enable switch + key status) and the start/stop streaming action.
 * WHY:   Streaming is secondary to recording, so it stays one click away instead of a
 *        sidebar card (progressive disclosure). Switches lock while live (STR-02); the start
 *        button names its blocker when disabled (STR-05). Keys are managed in Settings.
 * WHERE: main window record bar.
 */
export function GoLiveButton({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const [open, setOpen] = React.useState(false);
  const root = React.useRef<HTMLDivElement>(null);
  const dests = e.settings.streamDests;
  const enabledCount = dests.filter((d) => d.enabled).length;
  const block = blocker(dests);

  // Close on outside click and on Escape, like a native popover.
  React.useEffect(() => {
    if (!open) return;
    const onDown = (ev: PointerEvent) => {
      if (ev.target instanceof Node && !root.current?.contains(ev.target)) setOpen(false);
    };
    const onKey = (ev: KeyboardEvent) => ev.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  function toggleDest(i: number, enabled: boolean) {
    engine.update({ streamDests: dests.map((d, j) => (j === i ? { ...d, enabled } : d)) });
  }

  return (
    <div ref={root} className="relative">
      <Button variant="soft" size="lg" aria-expanded={open} aria-haspopup="dialog" onClick={() => setOpen((v) => !v)}>
        {e.live ? (
          <span className="rounded-[4px] bg-rec px-1.5 py-px text-[10.5px] font-bold tracking-wider text-rec-fg">
            LIVE
          </span>
        ) : (
          <Radio />
        )}
        {e.live ? (enabledCount > 1 ? `${enabledCount} destinations` : "Streaming") : "Go live"}
      </Button>

      {open && (
        <div
          role="dialog"
          aria-label="Live streaming"
          className="absolute right-0 bottom-full z-30 mb-2 w-[320px] origin-bottom-right animate-in rounded-[18px] border bg-popover p-3 text-popover-foreground shadow-float duration-150 ease-out-quick fade-in-0 zoom-in-95"
        >
          <CoachMark hint="go-live" title="Going live" className="mb-3 w-full shadow-none">
            Switch on where to stream, then Start. Add each stream key once in Settings › Live &amp; Replay; Cortex can
            record and stream at the same time.
          </CoachMark>
          <div className="flex items-baseline justify-between px-1 pb-2">
            <span className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
              Destinations
            </span>
            {e.live && <span className="text-[11px] text-muted-foreground">Locked while live</span>}
          </div>

          <ul className="space-y-1">
            {dests.map((d, i) => (
              <li key={`${d.service}-${i}`} className="flex items-center gap-2.5 rounded-[10px] px-1 py-1.5">
                <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-muted text-[12px] font-semibold">
                  {SERVICE_NAME[d.service].slice(0, 1)}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] leading-5">{SERVICE_NAME[d.service]}</span>
                  <span
                    className={cn(
                      "flex items-center gap-1 text-[11.5px] leading-4",
                      d.keyStored ? "text-ok" : "text-muted-foreground",
                    )}
                  >
                    <KeyRound className="size-3" aria-hidden />
                    {d.keyStored ? "Key stored in Keychain" : "No stream key yet"}
                  </span>
                </span>
                <Switch
                  aria-label={`Stream to ${SERVICE_NAME[d.service]}`}
                  checked={d.enabled}
                  onCheckedChange={(v) => toggleDest(i, v)}
                  disabled={e.live}
                />
              </li>
            ))}
          </ul>

          {e.live && e.streams.length > 0 && (
            <ul aria-label="Connection status" className="mt-1 space-y-0.5 border-t px-1 pt-2">
              {e.streams.map((st, i) => (
                <li
                  key={`${st.name}-${i}`}
                  className="flex items-baseline justify-between gap-2 text-[11.5px] leading-4"
                >
                  <span className="truncate text-muted-foreground">{st.name}</span>
                  <span className={cn("shrink-0", st.live ? "text-ok" : "text-warn")}>
                    {st.live ? "Live" : st.state}
                  </span>
                </li>
              ))}
            </ul>
          )}

          {!e.live && enabledCount > 1 && (
            <p className="px-1 pt-2 text-[11.5px] leading-4 text-muted-foreground">
              One encode feeds every destination, within each service's limits.
            </p>
          )}

          <div className="mt-3 space-y-2 border-t pt-3">
            {e.live ? (
              <Button variant="destructive" className="w-full" onClick={() => engine.toggleLive()}>
                Stop streaming
              </Button>
            ) : (
              <Button
                variant="tactile"
                className="w-full"
                disabled={block !== null}
                onClick={() => engine.toggleLive()}
              >
                {enabledCount > 1 ? `Go live on ${enabledCount} destinations` : "Go live"}
              </Button>
            )}
            {!e.live && block && <p className="text-center text-[11.5px] text-muted-foreground">{block}</p>}
            <button
              type="button"
              onClick={() => navigate("settings")}
              className="w-full rounded-md py-1 text-center text-[12px] text-accent-text outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
            >
              Stream keys and quality…
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
