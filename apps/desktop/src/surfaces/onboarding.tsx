import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";
import { ArrowLeft, ArrowRight, RotateCcw } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../lib/engine";
import type { Permission, PermissionReport } from "../lib/types";
import type { SurfaceProps } from "../prototype/surfaces";
import { TITLEBAR_H } from "../prototype/window-frame";
import { PermissionPanel, ReadyPanel, WelcomePanel } from "./onboarding/panels";
import type { ChipState } from "./onboarding/permission-chip";
import { OnboardingStage } from "./onboarding/stage";
import { PERMISSION_STEPS, STEPS, type StepId } from "./onboarding/steps";

/** The design harness shows review controls; the real app (Tauri) never does. */
const SHOW_PROTOTYPE_CONTROLS = !isTauri();
/** How often a permission step re-reads the grants (System Settings may change them). */
const POLL_MS = 1000;

/**
 * SOURCE OF TRUTH KEYWORDS: usePermissionReport, TCC polling, checkPermissions, screen relaunch, preflight
 * WHAT:  The privacy grants, re-read about once a second while `active`.
 * WHY:   Grants change in System Settings, outside the app; the chips must follow without
 *        a restart, and must learn when a Screen Recording grant waits for a relaunch.
 * WHERE: the onboarding permission steps.
 */
function usePermissionReport(active: boolean): PermissionReport | null {
  const [report, setReport] = React.useState<PermissionReport | null>(null);
  React.useEffect(() => {
    if (!active) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      const r = await engine.checkPermissions();
      if (!alive) return;
      setReport(r);
      timer = setTimeout(() => void poll(), POLL_MS);
    };
    void poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [active]);
  return active ? report : null;
}

/**
 * SOURCE OF TRUTH KEYWORDS: OnboardingWindow, onboarding, permissions flow, first run, TCC, welcome, setup
 * WHAT:  The first-run window: welcome, then Screen Recording (required), Microphone
 *        (recommended) and Camera (optional) permission steps, then a Ready summary.
 * WHY:   Recording cannot start without Screen Recording, and macOS only asks once.
 *        Asking in context, with a live mock and a status chip per step, gets the grants
 *        right the first time and explains the macOS quirks (quit & reopen, re-confirm).
 * WHERE: Shown on first launch and from Help › Permissions setup; prototype surface "onboarding".
 */
export function OnboardingWindow({ cameraStream, navigate }: SurfaceProps) {
  const e = useEngine();
  const [index, setIndex] = React.useState(0);
  // The permission whose prompt (or System Settings) is open right now.
  const [waiting, setWaiting] = React.useState<Permission | null>(null);
  // Harness only: make the next mock request come back denied, to review that state.
  const [denyNext, setDenyNext] = React.useState(false);
  // Ignore answers that arrive after a reset or after the window closed.
  const generation = React.useRef(0);
  React.useEffect(
    () => () => {
      generation.current += 1;
    },
    [],
  );

  const step: StepId = STEPS[index] ?? "welcome";
  const last = STEPS.length - 1;
  const permission = isPermissionStep(step) ? step : null;
  const titleId = React.useId();

  const report = usePermissionReport(permission !== null);
  const perms = report?.permissions ?? e.permissions;
  const chipFor = (p: Permission): ChipState => {
    if (perms[p] === "granted") return "granted";
    if (p === "screen" && report?.screenNeedsRelaunch) return "relaunch";
    return waiting === p ? "waiting" : perms[p];
  };
  const chips: Record<Permission, ChipState> = {
    screen: chipFor("screen"),
    microphone: chipFor("microphone"),
    camera: chipFor("camera"),
  };

  // Shows the macOS prompt (or System Settings) and waits for the answer. In the browser
  // harness the mock answers after a moment, denied when the reviewer asked for it.
  function request(p: Permission, viaSettings: boolean) {
    const gen = ++generation.current;
    setWaiting(p);
    if (viaSettings) engine.openSystemSettings(p);
    const outcome = denyNext && !viaSettings ? "denied" : "granted";
    void engine.requestPermission(p, outcome).then(() => {
      if (gen !== generation.current) return;
      setWaiting(null);
      setDenyNext(false);
    });
  }

  function resetPrototype() {
    generation.current += 1;
    setWaiting(null);
    setDenyNext(false);
    for (const p of ["screen", "microphone", "camera"] as const) engine.setPermission(p, "not-determined");
    setIndex(0);
  }

  const granted = permission ? perms[permission] === "granted" : true;
  const skippable = permission ? PERMISSION_STEPS[permission].need !== "required" : false;
  const canAdvance = waiting === null && (granted || skippable);
  const primaryLabel =
    step === "welcome"
      ? "Get started"
      : step === "ready"
        ? "Start recording"
        : granted || !skippable
          ? "Continue"
          : "Skip for now";

  const advance = React.useCallback(() => {
    if (!canAdvance) return;
    if (index >= last) navigate("main");
    else setIndex(index + 1);
  }, [canAdvance, index, last, navigate]);
  const back = React.useCallback(() => setIndex((i) => Math.max(0, i - 1)), []);

  // Enter / → advance, ← goes back. Enter on a focused control keeps its own meaning.
  React.useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.metaKey || ev.ctrlKey || ev.altKey) return;
      const t = ev.target;
      const onControl = t instanceof HTMLElement && t.closest("button, a, input, select, textarea") !== null;
      if (ev.key === "ArrowRight" || (ev.key === "Enter" && !onControl)) {
        ev.preventDefault();
        advance();
      } else if (ev.key === "ArrowLeft") {
        ev.preventDefault();
        back();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [advance, back]);

  const kicker =
    step === "ready" ? "All set" : step === "welcome" ? "Welcome to Cortex" : `Step ${index + 1} of ${STEPS.length}`;

  return (
    <section aria-labelledby={titleId} className="grid size-full grid-cols-[45%_1fr] bg-background text-foreground">
      <OnboardingStage step={step} engine={e} chips={chips} cameraStream={cameraStream} />

      <div className="relative flex min-h-0 flex-col" style={{ paddingTop: TITLEBAR_H }}>
        {/* Prototype-only controls; not part of the shipped window. */}
        {SHOW_PROTOTYPE_CONTROLS && (
          <div className="absolute top-2.5 right-3 flex items-center gap-1 rounded-full border border-dashed px-1 py-0.5 text-[11px] text-muted-foreground">
            <span className="pl-1.5">Prototype</span>
            <button
              type="button"
              onClick={() => setDenyNext((d) => !d)}
              aria-pressed={denyNext}
              className={cn(
                "rounded-full px-2 py-0.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
                denyNext && "bg-warn-tint text-warn",
              )}
            >
              Next answer: {denyNext ? "Don’t Allow" : "Allow"}
            </button>
            <button
              type="button"
              onClick={resetPrototype}
              className="flex items-center gap-1 rounded-full px-2 py-0.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
            >
              <RotateCcw className="size-3" /> Reset
            </button>
          </div>
        )}

        <div className="flex min-h-0 flex-1 flex-col justify-center overflow-y-auto px-10 pb-2">
          <div key={step} className="duration-300 ease-out-quick animate-in fade-in-0 slide-in-from-right-2">
            {step === "welcome" && <WelcomePanel kicker={kicker} titleId={titleId} />}
            {permission && (
              <PermissionPanel
                permission={permission}
                kicker={kicker}
                titleId={titleId}
                chip={chips[permission]}
                onRequest={() => request(permission, false)}
                onOpenSettings={() => request(permission, true)}
                onRelaunch={() => engine.relaunch()}
              />
            )}
            {step === "ready" && <ReadyPanel kicker={kicker} titleId={titleId} chips={chips} />}
          </div>
        </div>

        <footer className="flex h-[72px] shrink-0 items-center gap-3 px-10">
          <ol className="flex flex-1 items-center gap-1.5" aria-label={`Step ${index + 1} of ${STEPS.length}`}>
            {STEPS.map((s, i) => (
              <li
                key={s}
                aria-current={i === index ? "step" : undefined}
                className={cn(
                  "h-1.5 rounded-full transition-all duration-300 ease-out-quick",
                  i === index ? "w-6 bg-primary" : i < index ? "w-1.5 bg-primary/45" : "w-1.5 bg-border",
                )}
              />
            ))}
          </ol>
          {index > 0 && (
            <Button variant="ghost" size="lg" className="h-10 px-3" onClick={back}>
              <ArrowLeft data-icon="inline-start" />
              Back
            </Button>
          )}
          <div className="flex flex-col items-end">
            <Button
              variant={step === "ready" ? "record" : granted || step === "welcome" ? "tactile" : "soft"}
              size="lg"
              className="h-10 min-w-[120px] rounded-[var(--radius)] px-4"
              disabled={!canAdvance}
              onClick={advance}
              title={!canAdvance && !skippable && waiting === null ? "Allow screen recording to continue" : undefined}
            >
              {step === "ready" && <span className="size-2 rounded-full bg-rec-fg" aria-hidden />}
              {primaryLabel}
              {step !== "ready" && <ArrowRight data-icon="inline-end" />}
            </Button>
          </div>
        </footer>
      </div>
    </section>
  );
}

function isPermissionStep(s: StepId): s is Permission {
  return s === "screen" || s === "microphone" || s === "camera";
}
