// Checks the tour's per-platform table: every platform has copy for every step, only macOS
// gates Continue (on Screen Recording), the steps and copy follow each engine's capabilities,
// and a stored step resumes only when it is valid.
// Run with `pnpm --filter desktop test` (plain Node, no deps).
import {
  gatedPermission,
  PLATFORM_FACTS,
  resumeIndex,
  tourCopy,
  tourSteps,
  TOUR_STEPS,
  type TourCaps,
} from "./steps.ts";
import type { PlatformOs } from "../../lib/types";

const PLATFORMS: readonly PlatformOs[] = ["macos", "windows", "linux"];
let failures = 0;
function check(ok: boolean, what: string) {
  if (!ok) {
    failures += 1;
    console.error(`FAIL ${what}`);
  }
}

// What each engine reports today (mac_capabilities, windows_capabilities, linux_capabilities).
const CAPS: Record<PlatformOs, TourCaps> = {
  macos: {
    appCapture: true,
    sourcePicker: "app",
    captureExclusion: true,
    cameraTrack: true,
    quickTrim: true,
    replayBuffer: true,
    liveStreaming: true,
  },
  windows: {
    appCapture: false,
    sourcePicker: "app",
    captureExclusion: true,
    cameraTrack: false,
    quickTrim: false,
    replayBuffer: false,
    liveStreaming: false,
  },
  linux: {
    appCapture: false,
    sourcePicker: "system",
    captureExclusion: false,
    cameraTrack: true,
    quickTrim: false,
    replayBuffer: false,
    liveStreaming: false,
  },
};

for (const p of PLATFORMS) {
  const copy = tourCopy(p, CAPS[p]);
  for (const step of TOUR_STEPS) {
    const c = copy[step];
    check(c.kicker.length > 0 && c.title.length > 0 && c.lede.length > 0, `${p}/${step} has copy`);
  }
  const f = PLATFORM_FACTS[p];
  check(f.recordKeys.length > 0 && f.replayKeys.length > 0, `${p} has shortcut keys`);
  check(f.permissions.length > 0, `${p} lists permissions`);
  for (const step of TOUR_STEPS) {
    const gate = gatedPermission(p, step);
    const expected = p === "macos" && step === "permissions" ? "screen" : null;
    check(gate === expected, `${p}/${step} gate is ${String(expected)} (got ${String(gate)})`);
  }
}

const mac = tourCopy("macos", CAPS.macos);
const win = tourCopy("windows", CAPS.windows);
const linux = tourCopy("linux", CAPS.linux);
check(!mac.shortcut.lede.includes("Ctrl") && mac.shortcut.lede.includes("⇧⌘R"), "macOS shortcut copy uses ⇧⌘R");
check(win.shortcut.lede.includes("Ctrl+Shift+R"), "Windows shortcut copy uses Ctrl+Shift+R");
check(linux.shortcut.lede.includes("Ctrl+Shift+R"), "Linux shortcut copy uses Ctrl+Shift+R");
check(linux.permissions.title.includes("share"), "Linux permissions step is about the portal");

// Steps follow the capabilities: no Quick trim or replay/live step where the engine lacks them.
check(tourSteps(CAPS.macos).length === TOUR_STEPS.length, "macOS shows every step");
for (const p of ["windows", "linux"] as const) {
  const steps = tourSteps(CAPS[p]);
  check(!steps.includes("trim"), `${p} has no Quick trim step`);
  check(!steps.includes("replay-live"), `${p} has no replay and live step`);
  check(steps[0] === "welcome" && steps.at(-1) === "done", `${p} still starts with welcome and ends with done`);
}
check(tourSteps({ ...CAPS.windows, liveStreaming: true }).includes("replay-live"), "Go live alone keeps the step");

// Copy never promises what the engine can't do.
check(mac.capture.lede.includes("every window of one app"), "macOS mentions app capture");
check(!win.capture.lede.includes("every window of one app"), "Windows does not promise app capture");
check(linux.capture.lede.includes("system dialog"), "Linux explains the system picker");
check(!linux.capture.lede.includes("never records its own windows"), "Linux does not promise capture exclusion");
check(!win.camera.lede.includes("own track"), "Windows does not promise a camera track");
check(linux.camera.lede.includes("own track"), "Linux keeps the camera track");
check(PLATFORM_FACTS.windows.defaultFolder === "~/Videos/Cortex", "Windows default folder matches rec-core");

check(resumeIndex(null) === 0, "no stored step starts at 0");
check(resumeIndex("3") === 3, "a stored step resumes");
check(resumeIndex("42") === 0, "an out-of-range step starts at 0");
check(resumeIndex("abc") === 0, "garbage starts at 0");
check(resumeIndex("-1") === 0, "a negative step starts at 0");
check(resumeIndex("7", 7) === 0, "a step past a shorter tour starts at 0");

if (failures > 0) process.exit(1);
console.log(`tour steps: ok (${PLATFORMS.length} platforms × ${TOUR_STEPS.length} steps)`);
