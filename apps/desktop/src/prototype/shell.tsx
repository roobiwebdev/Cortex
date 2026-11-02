import * as React from "react";
import { Camera, CameraOff, Moon, Monitor, Sun } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../lib/mock-engine";
import type { ThemePref } from "../lib/types";
import { ACCENT_PRESETS, useStudioTheme } from "../lib/use-studio-theme";
import { DesktopBackdrop } from "./desktop-backdrop";
import { SURFACES, type SurfaceId } from "./surfaces";
import { WindowFrame } from "./window-frame";

/**
 * SOURCE OF TRUTH KEYWORDS: PrototypeShell, design review, surface switcher, theme toggle
 * WHAT:  The design-review harness: pick a surface, switch theme/accent, use a real
 *        camera, and drive the mock engine.
 * WHY:   The user reviews every screen in a browser before the backend is built.
 * WHERE: apps/desktop dev server (http://localhost:3002). Not shipped in the app.
 */
export function PrototypeShell() {
  const e = useEngine();
  useStudioTheme(e.settings.theme, e.settings.accentHue);
  const [active, setActive] = React.useState<SurfaceId>(() => readHash());
  const [camera, setCamera] = React.useState<MediaStream | null>(null);

  React.useEffect(() => {
    const onHash = () => setActive(readHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  const navigate = React.useCallback((id: SurfaceId) => {
    window.location.hash = id;
    setActive(id);
  }, []);

  // Global shortcuts, mirroring the app: ⇧⌘R record/stop, ⇧⌘S save replay, Esc cancels countdown.
  React.useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.metaKey && ev.shiftKey && ev.key.toLowerCase() === "r") {
        ev.preventDefault();
        engine.toggleRecord();
      } else if (ev.metaKey && ev.shiftKey && ev.key.toLowerCase() === "s") {
        ev.preventDefault();
        engine.saveReplay();
      } else if (ev.key === "Escape") engine.cancelCountdown();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  async function toggleCamera() {
    if (camera) {
      for (const t of camera.getTracks()) t.stop();
      setCamera(null);
      return;
    }
    try {
      setCamera(await navigator.mediaDevices.getUserMedia({ video: { width: 1280, height: 720 } }));
    } catch {
      setCamera(null);
    }
  }

  const surface = SURFACES.find((s) => s.id === active) ?? SURFACES[0];
  if (!surface) return null;
  const { Component } = surface;
  const groups = ["Windows", "Floating", "Flows"] as const;

  return (
    <div className="flex h-full bg-muted text-foreground">
      <aside className="flex w-60 shrink-0 flex-col gap-5 border-r bg-card p-4">
        <div>
          <div className="font-display text-xl font-bold tracking-tight">Cortex</div>
          <div className="text-xs text-muted-foreground">Desktop UI · design review</div>
        </div>
        {groups.map((g) => (
          <nav key={g} className="space-y-0.5">
            <div className="px-2 pb-1 text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
              {g}
            </div>
            {SURFACES.filter((s) => s.group === g).map((s) => (
              <button
                key={s.id}
                type="button"
                onClick={() => navigate(s.id)}
                className={cn(
                  "flex w-full items-center rounded-[10px] px-2 py-1.5 text-left text-[13px] transition-colors",
                  s.id === active ? "bg-accent font-medium text-accent-foreground" : "hover:bg-muted",
                )}
              >
                {s.label}
              </button>
            ))}
          </nav>
        ))}

        <div className="mt-auto space-y-3 border-t pt-4">
          <Segmented<ThemePref>
            aria-label="Theme"
            size="sm"
            className="w-full"
            value={e.settings.theme}
            onValueChange={(theme) => engine.update({ theme })}
            options={[
              { value: "system", label: <Monitor />, title: "System" },
              { value: "light", label: <Sun />, title: "Light" },
              { value: "dark", label: <Moon />, title: "Dark" },
            ]}
          />
          <div className="flex justify-between">
            {ACCENT_PRESETS.map((a) => (
              <button
                key={a.hue}
                type="button"
                title={a.label}
                aria-label={`Accent ${a.label}`}
                onClick={() => engine.update({ accentHue: a.hue })}
                className={cn(
                  "size-6 rounded-full ring-offset-2 ring-offset-card transition-shadow",
                  e.settings.accentHue === a.hue && "ring-2 ring-ring",
                )}
                style={{ background: `hsl(${a.hue} 60% 45%)` }}
              />
            ))}
          </div>
          <Button variant="soft" size="sm" className="w-full" onClick={toggleCamera}>
            {camera ? <CameraOff /> : <Camera />}
            {camera ? "Stop my camera" : "Use my camera"}
          </Button>
          <div className="grid grid-cols-2 gap-1.5">
            <Button variant="outline" size="xs" onClick={() => engine.demoToast("saved")}>
              Toast: saved
            </Button>
            <Button variant="outline" size="xs" onClick={() => engine.demoToast("warning")}>
              Toast: warn
            </Button>
            <Button variant="outline" size="xs" onClick={() => engine.toggleRecord()}>
              {e.status === "idle" ? "Start rec" : "Stop rec"}
            </Button>
            <Button
              variant="outline"
              size="xs"
              onClick={() => engine.setPermission("screen", e.permissions.screen === "granted" ? "denied" : "granted")}
            >
              Screen perm: {e.permissions.screen === "granted" ? "on" : "off"}
            </Button>
          </div>
        </div>
      </aside>

      <main className="relative min-w-0 flex-1">
        {surface.frame.kind === "window" ? (
          <DesktopBackdrop onStatusItem={() => navigate("menubar")} statusItemActive={false}>
            <div className="absolute inset-0 top-7 grid place-items-center overflow-auto bg-background/30 p-8 backdrop-blur-md">
              <WindowFrame width={surface.frame.width} height={surface.frame.height}>
                <Component cameraStream={camera} navigate={navigate} />
              </WindowFrame>
            </div>
          </DesktopBackdrop>
        ) : (
          <DesktopBackdrop onStatusItem={() => navigate("menubar")} statusItemActive={active === "menubar"}>
            <Component cameraStream={camera} navigate={navigate} />
          </DesktopBackdrop>
        )}
      </main>
    </div>
  );
}

function readHash(): SurfaceId {
  const h = window.location.hash.slice(1);
  return SURFACES.find((s) => s.id === h)?.id ?? "main";
}
