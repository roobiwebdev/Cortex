import * as React from "react";

/**
 * SOURCE OF TRUTH KEYWORDS: useFlash, transient feedback, copied state, revealed state, button feedback
 * WHAT:  Remembers which action just ran for `ms` so a button can say "Copied" briefly.
 * WHY:   Reveal and Copy have no visible result inside a floating surface; a short label
 *        swap confirms the click without another toast.
 * WHERE: ToastStack actions, MenuBarPanel last-recording actions.
 */
export function useFlash(ms = 1400) {
  const [flashed, setFlashed] = React.useState<string | null>(null);
  const timer = React.useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  React.useEffect(() => () => clearTimeout(timer.current), []);
  const flash = React.useCallback(
    (key: string) => {
      clearTimeout(timer.current);
      setFlashed(key);
      timer.current = setTimeout(() => setFlashed(null), ms);
    },
    [ms],
  );
  return [flashed, flash] as const;
}
