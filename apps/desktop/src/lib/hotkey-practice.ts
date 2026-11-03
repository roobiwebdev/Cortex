import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";

import { commands, events } from "../ipc/bindings";

const inTauri = isTauri();
/** Harness only: the tour step that is practising the record shortcut. */
let harnessHandler: (() => void) | null = null;

/**
 * SOURCE OF TRUTH KEYWORDS: useHotkeyPractice, hotkey practice, record shortcut, tour://hotkey-pressed, set_hotkey_practice
 * WHAT:  While `active`, a press of the global record shortcut calls `onPress` instead of
 *        starting a recording.
 * WHY:   The global hotkey is a Carbon hotkey: the webview never sees the key event. The
 *        shell routes it to the tour while the tour asks (`set_hotkey_practice`), so the
 *        user can try it safely. In the harness the prototype shell's key handler asks
 *        `catchHarnessHotkey()` first.
 * WHERE: the onboarding tour's shortcut step.
 */
export function useHotkeyPractice(active: boolean, onPress: () => void) {
  const press = React.useEffectEvent(onPress);
  React.useEffect(() => {
    if (!active) return;
    if (!inTauri) {
      const handler = () => press();
      harnessHandler = handler;
      return () => {
        if (harnessHandler === handler) harnessHandler = null;
      };
    }
    let alive = true;
    let unlisten: (() => void) | undefined;
    void events.tourHotkeyPressed
      .listen(() => press())
      .then((u) => {
        if (alive) unlisten = u;
        else u();
      });
    commands.setHotkeyPractice(true).catch(() => undefined);
    return () => {
      alive = false;
      unlisten?.();
      commands.setHotkeyPractice(false).catch(() => undefined);
    };
  }, [active]);
}

/** Harness: true when a tour step took the record shortcut (the shell then does nothing). */
export function catchHarnessHotkey(): boolean {
  if (!harnessHandler) return false;
  harnessHandler();
  return true;
}
