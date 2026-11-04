import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowLeft, ArrowRight, RotateCcw } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";
import { cn } from "@starter/ui/lib/utils";

import { BrandMark } from "../components/brand-mark";
import { engine, useEngine } from "../lib/engine";
import { useHotkeyPractice } from "../lib/hotkey-practice";
import { PLATFORMS, setHarnessPlatform, usePlatform } from "../lib/platform";
import { playBlip } from "../lib/sounds";
import type { Permission, PermissionReport, PlatformOs } from "../lib/types";
import { onPageShown } from "../lib/visibility";
import type { SurfaceProps } from "../prototype/surfaces";
import { TITLEBAR_H } from "../prototype/window-frame";
import {
  CameraBody,
  CaptureBody,
  DoneBody,
  FilesBody,
  PermissionsBody,
  ReplayLiveBody,
  ShortcutBody,
  StepHeader,
  TrimBody,
  WelcomeBody,
} from "./onboarding/panels";
import type { ChipState } from "./onboarding/permission-chip";
import { OnboardingStage, type CaptureMode } from "./onboarding/stage";
import { gatedPermission, PLATFORM_FACTS, resumeIndex, tourCopy, tourSteps, type TourStepId } from "./onboarding/steps";

const inTauri = isTauri();
/** The harness shows review controls (platform, next answer, reset) unless `?clean` is set. */
const SHOW_PROTOTYPE_CONTROLS =
  !inTauri && typeof location !== "undefined" && !new URLSearchParams(location.search).has("clean");
/** How often the permissions step re-reads the grants (System Settings may change them). */
const POLL_MS = 1000;
/** The step to come back to after a relaunch (Screen Recording may need one). */
const STEP_KEY = "cortex.tour.step";
const PLATFORM_LABEL: Record<PlatformOs, string> = { macos: "macOS", windows: "Windows", linux: "Linux" };

function readStoredStep(): string | null {
  try {
    return localStorage.getItem(STEP_KEY);
  } catch {
    return null;
  }
}

function storeStep(index: number | null) {
  try {
    if (index === null) localStorage.removeItem(STEP_KEY);
    else localStorage.setItem(STEP_KEY, String(index));
  } catch {
    // Storage may be unavailable; the tour then starts from the top after a relaunch.
  }
}

/**
 * SOURCE OF TRUTH KEYWORDS: usePermissionReport, TCC polling, checkPermissions, screen relaunch, preflight
 * WHAT:  The privacy grants, re-read about once a second while `active`.
 * WHY:   Grants change in System Settings, outside the app; the chips must follow without
 *        a restart, and must learn when a Screen Recording grant waits for a relaunch.
 * WHERE: the tour's permissions and last steps.
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
 * SOURCE OF TRUTH KEYWORDS: OnboardingWindow, first-run tour, onboarding, guided tour, permissions, Show the Tour, onboardingCompleted
 * WHAT:  The first-run tour (up to nine steps) in the 880×560 onboarding window: welcome, permissions,
 *        what to record, camera, the record shortcut, files, Quick trim, replay and live, done.
 *        A live animated stage on the left; kicker, headline, lede and the real control on
 *        the right; a dot pager and Back / Continue below.
 * WHY:   People learn a recorder fastest by trying it. Each step pairs a short explanation
 *        with the real setting or action, and only macOS's Screen Recording grant blocks
 *        Continue, because nothing records without it. Finishing, skipping or closing the
 *        window stores `onboardingCompleted`, so the tour opens by itself only once.
 * WHERE: opened at first launch and from Help › Show the Tour or Settings › General;
 *        prototype surface "onboarding". Copy per platform lives in `onboarding/steps.ts`.
 */
export function OnboardingWindow({ cameraStream, navigate }: SurfaceProps) {
  const e = useEngine();
  const platform = usePlatform();
  const completed = e.settings.onboardingCompleted;
  // Only the steps this engine can do: no Quick trim or replay step where they don't exist.
  const caps = e.capabilities;
  const steps = React.useMemo(() => tourSteps(caps), [caps]);
  const [index, setIndex] = React.useState(() => (completed ? 0 : resumeIndex(readStoredStep(), steps.length)));
  const [captureMode, setCaptureMode] = React.useState<CaptureMode>("area");
  const [hotkeyPressed, setHotkeyPressed] = React.useState(false);
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

  const step: TourStepId = steps[index] ?? "welcome";
  const last = steps.length - 1;
  const copy = React.useMemo(() => tourCopy(platform, caps), [platform, caps])[step];
  const facts = PLATFORM_FACTS[platform];
  const titleId = React.useId();

  // Reopened from the menu after it was finished: start again from the top.
  React.useEffect(
    () =>
      onPageShown(() => {
        if (completed) {
          setIndex((i) => (i === last ? 0 : i));
          setHotkeyPressed(false);
        }
      }),
    [completed, last],
  );
  // Survive a relaunch mid-tour; forget the place once it is done.
  React.useEffect(() => storeStep(completed ? null : index), [completed, index]);

  const report = usePermissionReport(step === "permissions" || step === "done");
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

  useHotkeyPractice(step === "shortcut", () => {
    setHotkeyPressed(true);
    if (e.settings.sounds) playBlip(1, 0.09);
  });

  // Shows the system prompt (or the settings app) and waits for the answer. In the browser
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
    setHotkeyPressed(false);
    for (const p of ["screen", "microphone", "camera"] as const) engine.setPermission(p, "not-determined");
    setIndex(0);
  }

  const gate = gatedPermission(platform, step);
  const canAdvance = waiting === null && (gate === null || perms[gate] === "granted");
  const primaryLabel = step === "welcome" ? "Get started" : step === "done" ? "Start recording" : "Continue";

  /** Finish (or skip) the tour: remember it, close the window, optionally start recording. */
  const finish = React.useCallback(
    (startRecording: boolean) => {
      storeStep(null);
      engine.update({ onboardingCompleted: true });
      navigate("main");
      if (startRecording) engine.start();
      if (inTauri)
        getCurrentWindow()
          .hide()
          .catch(() => undefined);
    },
    [navigate],
  );

  const go = React.useCallback(
    (next: number) => {
      const to = Math.max(0, Math.min(last, next));
      if (to === index) return;
      setIndex(to);
      // A soft blip per step; never while capturing (system audio would record it).
      if (e.settings.sounds && e.status === "idle") playBlip(to / last);
    },
    [e.settings.sounds, e.status, index, last],
  );
  const advance = React.useCallback(() => {
    if (!canAdvance) return;
    if (index >= last) finish(true);
    else go(index + 1);
  }, [canAdvance, finish, go, index, last]);
  const back = React.useCallback(() => go(index - 1), [go, index]);

  // Enter / → advance, ← goes back. Enter on a focused control keeps its own meaning.
  React.useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.metaKey || ev.ctrlKey || ev.altKey) return;
      const t = ev.target;
      const onControl =
        t instanceof HTMLElement && t.closest("button, a, input, select, textarea, [role=radio]") !== null;
      if (ev.key === "ArrowRight" || (ev.key === "Enter" && !onControl)) {
        ev.preventDefault();
        advance();
      } else if (ev.key === "ArrowLeft" && !onControl) {
        ev.preventDefault();
        back();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [advance, back]);

  const kicker =
    step === "welcome" || step === "done" ? copy.kicker : `Step ${index + 1} of ${steps.length} · ${copy.kicker}`;

  return (
    <section
      aria-labelledby={titleId}
      className="grid size-full grid-cols-[46%_minmax(0,1fr)] bg-background text-foreground"
    >
      <OnboardingStage
        step={step}
        platform={platform}
        engine={e}
        chips={chips}
        cameraStream={cameraStream}
        captureMode={captureMode}
        hotkeyPressed={hotkeyPressed}
      />

      <div className="relative flex min-h-0 flex-col" style={{ paddingTop: TITLEBAR_H }}>
        <div className="absolute top-2.5 right-3 flex items-center gap-2">
          {/* Prototype-only controls; not part of the shipped window. */}
          {SHOW_PROTOTYPE_CONTROLS && (
            <div className="flex items-center gap-1 rounded-full border border-dashed px-1 py-0.5 text-[11px] text-muted-foreground">
              <Segmented<PlatformOs>
                aria-label="Platform"
                size="sm"
                value={platform}
                onValueChange={setHarnessPlatform}
                options={PLATFORMS.map((p) => ({ value: p, label: PLATFORM_LABEL[p] }))}
              />
              <button
                type="button"
                onClick={() => setDenyNext((d) => !d)}
                aria-pressed={denyNext}
                title="Next answer to a permission prompt"
                className={cn(
                  "rounded-full px-2 py-0.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
                  denyNext && "bg-warn-tint text-warn",
                )}
              >
                {denyNext ? "Deny" : "Allow"}
              </button>
              <button
                type="button"
                onClick={resetPrototype}
                aria-label="Reset the tour"
                className="grid size-6 place-items-center rounded-full outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
              >
                <RotateCcw className="size-3" />
              </button>
            </div>
          )}
          {step !== "done" && (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-foreground"
              onClick={() => finish(false)}
            >
              Skip tour
            </Button>
          )}
        </div>

        <div className="flex min-h-0 flex-1 flex-col justify-center overflow-y-auto px-10 pb-2">
          <div
            key={`${platform}-${step}`}
            className="duration-300 ease-out-quick animate-in fade-in-0 slide-in-from-right-2"
          >
            {step === "welcome" && <BrandMark className="mb-4 size-10" />}
            <StepHeader copy={copy} kicker={kicker} titleId={titleId} />
            {step === "welcome" && <WelcomeBody platform={platform} />}
            {step === "permissions" && (
              <PermissionsBody
                platform={platform}
                chips={chips}
                onRequest={(p) => request(p, false)}
                onOpenSettings={(p) => request(p, true)}
                onRelaunch={() => engine.relaunch()}
              />
            )}
            {step === "capture" && (
              <CaptureBody
                mode={captureMode}
                caps={caps}
                onMode={setCaptureMode}
                onTryArea={() => navigate("region")}
              />
            )}
            {step === "camera" && <CameraBody e={e} />}
            {step === "shortcut" && <ShortcutBody platform={platform} pressed={hotkeyPressed} />}
            {step === "files" && <FilesBody platform={platform} outputDir={e.settings.outputDir} />}
            {step === "trim" && <TrimBody onOpenTrim={() => navigate("trim")} />}
            {step === "replay-live" && (
              <ReplayLiveBody e={e} platform={platform} onOpenLiveSettings={() => navigate("settings")} />
            )}
            {step === "done" && <DoneBody platform={platform} chips={chips} />}
          </div>
        </div>

        <footer className="flex h-[72px] shrink-0 items-center gap-3 px-10">
          <ol className="flex flex-1 items-center gap-1.5" aria-label={`Step ${index + 1} of ${steps.length}`}>
            {steps.map((s, i) => (
              <li key={s} aria-current={i === index ? "step" : undefined}>
                <button
                  type="button"
                  aria-label={`Go to step ${i + 1}`}
                  // Only steps already passed can be jumped to, so a gate is never skipped.
                  disabled={i > index}
                  onClick={() => go(i)}
                  className={cn(
                    "block h-1.5 rounded-full transition-all duration-300 ease-out-quick outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    i === index
                      ? "w-6 bg-primary"
                      : i < index
                        ? "w-1.5 bg-primary/45 hover:bg-primary/70"
                        : "w-1.5 bg-border",
                  )}
                />
              </li>
            ))}
          </ol>
          {index > 0 && (
            <Button variant="ghost" size="lg" className="h-10 px-3" onClick={back}>
              <ArrowLeft data-icon="inline-start" />
              Back
            </Button>
          )}
          <Button
            variant={step === "done" ? "record" : "tactile"}
            size="lg"
            className="h-10 min-w-[120px] rounded-[var(--radius)] px-4"
            disabled={!canAdvance}
            onClick={advance}
            title={
              !canAdvance && waiting === null
                ? `Allow ${facts.permissions[0]?.name ?? "access"} to continue`
                : undefined
            }
          >
            {step === "done" && <span className="size-2 rounded-full bg-rec-fg" aria-hidden />}
            {primaryLabel}
            {step !== "done" && <ArrowRight data-icon="inline-end" />}
          </Button>
        </footer>
      </div>
    </section>
  );
}
