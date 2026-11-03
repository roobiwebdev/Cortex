// Checks the tour's per-platform table: every platform has copy for every step, only macOS
// gates Continue (on Screen Recording), and a stored step resumes only when it is valid.
// Run with `pnpm --filter desktop test` (plain Node, no deps).
import { gatedPermission, PLATFORM_FACTS, resumeIndex, TOUR_COPY, TOUR_STEPS } from "./steps.ts";
import type { Platform } from "../../lib/types";

const PLATFORMS: readonly Platform[] = ["macos", "windows", "linux"];
let failures = 0;
function check(ok: boolean, what: string) {
  if (!ok) {
    failures += 1;
    console.error(`FAIL ${what}`);
  }
}

for (const p of PLATFORMS) {
  for (const step of TOUR_STEPS) {
    const c = TOUR_COPY[p][step];
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
check(!TOUR_COPY.macos.shortcut.lede.includes("Ctrl"), "macOS shortcut copy uses ⌘");
check(TOUR_COPY.windows.shortcut.lede.includes("Ctrl+Shift+R"), "Windows shortcut copy uses Ctrl+Shift+R");
check(TOUR_COPY.linux.permissions.title.includes("share"), "Linux permissions step is about the portal");

check(resumeIndex(null) === 0, "no stored step starts at 0");
check(resumeIndex("3") === 3, "a stored step resumes");
check(resumeIndex("42") === 0, "an out-of-range step starts at 0");
check(resumeIndex("abc") === 0, "garbage starts at 0");
check(resumeIndex("-1") === 0, "a negative step starts at 0");

if (failures > 0) process.exit(1);
console.log(`tour steps: ok (${PLATFORMS.length} platforms × ${TOUR_STEPS.length} steps)`);
