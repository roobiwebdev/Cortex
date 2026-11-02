import * as React from "react";
import { RotateCcw } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { cn } from "@starter/ui/lib/utils";

import { SettingsGroup, SettingsPage } from "./parts";

/** A shortcut as modifier flags plus one key, so it can be compared and drawn as glyphs. */
interface Combo {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  key: string;
}
type ActionId = "record" | "replay" | "pause" | "camera" | "mic" | "hud";

const c = (key: string, mods: Partial<Omit<Combo, "key">> = {}): Combo => ({
  ctrl: false,
  alt: false,
  shift: false,
  meta: false,
  ...mods,
  key,
});

const ACTIONS: ReadonlyArray<{ id: ActionId; label: string; hint: string; combo: Combo | null }> = [
  {
    id: "record",
    label: "Start / stop recording",
    hint: "Works from any app.",
    combo: c("R", { shift: true, meta: true }),
  },
  {
    id: "replay",
    label: "Save replay",
    hint: "Saves the replay buffer as an MP4.",
    combo: c("S", { shift: true, meta: true }),
  },
  {
    id: "pause",
    label: "Pause / resume",
    hint: "Paused time is cut from the file.",
    combo: c("P", { shift: true, meta: true }),
  },
  { id: "camera", label: "Toggle camera", hint: "Shows or hides the overlay.", combo: null },
  { id: "mic", label: "Toggle microphone", hint: "Mutes without stopping the track.", combo: null },
  { id: "hud", label: "Show HUD", hint: "Brings the floating timer back.", combo: c("H", { alt: true, meta: true }) },
];

/** System and common app shortcuts we refuse, with the reason shown inline. */
const RESERVED: ReadonlyArray<{ combo: Combo; owner: string }> = [
  { combo: c("Q", { meta: true }), owner: "Quit" },
  { combo: c("W", { meta: true }), owner: "Close Window" },
  { combo: c("3", { shift: true, meta: true }), owner: "macOS Screenshot" },
  { combo: c("4", { shift: true, meta: true }), owner: "macOS Screenshot" },
  { combo: c("5", { shift: true, meta: true }), owner: "macOS Screenshot" },
  { combo: c(" ", { meta: true }), owner: "Spotlight" },
  { combo: c("Tab", { meta: true }), owner: "App Switcher" },
];

const same = (a: Combo, b: Combo) =>
  a.key === b.key && a.ctrl === b.ctrl && a.alt === b.alt && a.shift === b.shift && a.meta === b.meta;

/** Glyphs in Apple's order: ⌃ ⌥ ⇧ ⌘ then the key. */
function glyphs(k: Combo) {
  const out: string[] = [];
  if (k.ctrl) out.push("⌃");
  if (k.alt) out.push("⌥");
  if (k.shift) out.push("⇧");
  if (k.meta) out.push("⌘");
  out.push(k.key === " " ? "Space" : k.key);
  return out;
}

function keyName(e: React.KeyboardEvent) {
  // e.code survives ⌥ producing special characters (⌥R = ®).
  if (e.code.startsWith("Key")) return e.code.slice(3);
  if (e.code.startsWith("Digit")) return e.code.slice(5);
  if (e.code === "Space") return " ";
  if (/^F\d+$/.test(e.code)) return e.code;
  const named: Record<string, string> = {
    Tab: "Tab",
    Enter: "↩",
    Backspace: "⌫",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
  };
  return named[e.code] ?? null;
}

/** Shortcuts: global hotkeys with a recorder. Custom hotkeys are deferred in Rec; this recorder is prototype-local. */
export function ShortcutsSection() {
  // prototype-local: needs a `shortcuts: Record<ActionId, Combo | null>` setting and global hotkey registration.
  const [map, setMap] = React.useState<Record<ActionId, Combo | null>>(() => {
    const m: Record<ActionId, Combo | null> = {
      record: null,
      replay: null,
      pause: null,
      camera: null,
      mic: null,
      hud: null,
    };
    for (const a of ACTIONS) m[a.id] = a.combo;
    return m;
  });
  const [recording, setRecording] = React.useState<ActionId | null>(null);
  const [error, setError] = React.useState<{ id: ActionId; text: string } | null>(null);

  function capture(id: ActionId, e: React.KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return setRecording(null);
    if ((e.key === "Backspace" || e.key === "Delete") && !e.metaKey && !e.ctrlKey && !e.altKey) {
      setMap((m) => ({ ...m, [id]: null }));
      return setRecording(null);
    }
    const key = keyName(e);
    if (!key) return; // modifiers alone: keep waiting
    const combo: Combo = { ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey, key };
    if (!combo.ctrl && !combo.alt && !combo.meta) {
      return setError({ id, text: "Add ⌘, ⌥ or ⌃ so typing in other apps isn't hijacked." });
    }
    const reserved = RESERVED.find((r) => same(r.combo, combo));
    if (reserved) return setError({ id, text: `${glyphs(combo).join("")} is used by ${reserved.owner}.` });
    const clash = ACTIONS.find((a) => a.id !== id && map[a.id] && same(map[a.id] ?? combo, combo));
    if (clash) return setError({ id, text: `Already used for “${clash.label}”.` });
    setError(null);
    setMap((m) => ({ ...m, [id]: combo }));
    setRecording(null);
  }

  return (
    <SettingsPage title="Shortcuts" subtitle="Global keys that work while other apps are in front.">
      <SettingsGroup
        title="Recording"
        footer="Click a shortcut, then type the new keys. Esc cancels, Delete clears. Saved in this prototype only."
      >
        {ACTIONS.map((a) => {
          const combo = map[a.id];
          const active = recording === a.id;
          const err = error?.id === a.id ? error.text : null;
          const changed = !(combo === a.combo || (combo && a.combo && same(combo, a.combo)));
          return (
            <div key={a.id} className="flex min-h-[52px] items-center gap-4 px-4 py-2.5">
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium">{a.label}</div>
                <div className={cn("text-xs", err ? "text-warn" : "text-muted-foreground")} aria-live="polite">
                  {err ?? a.hint}
                </div>
              </div>
              {changed && (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Reset ${a.label} shortcut`}
                  title="Reset to default"
                  onClick={() => {
                    setMap((m) => ({ ...m, [a.id]: a.combo }));
                    setError(null);
                  }}
                >
                  <RotateCcw />
                </Button>
              )}
              <button
                type="button"
                aria-label={`${a.label} shortcut: ${combo ? glyphs(combo).join(" ") : "none"}. Click to change.`}
                onClick={() => {
                  setError(null);
                  setRecording(active ? null : a.id);
                }}
                onKeyDown={(e) => active && capture(a.id, e)}
                onBlur={() => active && setRecording(null)}
                className={cn(
                  "flex h-8 min-w-36 items-center justify-center gap-1 rounded-[10px] border bg-card px-2.5 text-xs outline-none transition-colors hover:border-ring/50 focus-visible:ring-3 focus-visible:ring-ring/30",
                  active && "border-ring bg-accent text-accent-text ring-3 ring-ring/20",
                  err && active && "border-warn ring-warn/20",
                )}
              >
                {active ? (
                  <span className="font-medium">Type shortcut…</span>
                ) : combo ? (
                  glyphs(combo).map((g, i) => <Kbd key={i}>{g}</Kbd>)
                ) : (
                  <span className="text-muted-foreground">Record shortcut</span>
                )}
              </button>
            </div>
          );
        })}
      </SettingsGroup>
    </SettingsPage>
  );
}
