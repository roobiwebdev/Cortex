import * as React from "react";

import { clamp } from "./time";

/**
 * SOURCE OF TRUTH KEYWORDS: usePlayback, simulated playback, shuttle, JKL, play rate, playhead time
 * WHAT:  Simulated player clock for the trim window: play/pause, J/K/L shuttle rates,
 *        seeking, and stopping at the out point (or the in point when playing backwards).
 * WHY:   The prototype has no real <video>; the real app drives the same API from the
 *        media element's currentTime (NEW-TR-02), so the UI does not change.
 * WHERE: TrimWindow.
 */
export function usePlayback({ duration, inT, outT }: { duration: number; inT: number; outT: number }) {
  const [time, setTime] = React.useState(0);
  // 0 = paused; negative plays backwards (J).
  const [rate, setRate] = React.useState(0);
  const timeRef = React.useRef(0);

  React.useEffect(() => {
    if (rate === 0) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      let next = timeRef.current + dt * rate;
      let stop = false;
      if (rate > 0 && next >= outT) {
        next = outT;
        stop = true;
      } else if (rate < 0 && next <= inT) {
        next = inT;
        stop = true;
      }
      timeRef.current = next;
      setTime(next);
      if (stop) setRate(0);
      else raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [rate, inT, outT]);

  const seek = React.useCallback(
    (t: number) => {
      const next = clamp(t, 0, duration);
      timeRef.current = next;
      setTime(next);
    },
    [duration],
  );

  const play = React.useCallback(() => {
    // Starting at or past the out point (or before the in point) replays the selection.
    const t = timeRef.current;
    if (t >= outT - 0.01 || t < inT) seek(inT);
    setRate(1);
  }, [inT, outT, seek]);

  const pause = React.useCallback(() => setRate(0), []);
  const toggle = React.useCallback(() => (rate === 0 ? play() : pause()), [rate, play, pause]);

  // J/L: first press plays at 1×, each further press doubles up to 4×.
  const forward = React.useCallback(() => {
    if (rate <= 0) {
      play();
      return;
    }
    setRate(Math.min(4, rate * 2));
  }, [rate, play]);
  const backward = React.useCallback(() => {
    if (timeRef.current <= inT + 0.01 && rate >= 0) return;
    setRate(rate >= 0 ? -1 : Math.max(-4, rate * 2));
  }, [rate, inT]);

  return { time, rate, playing: rate !== 0, seek, play, pause, toggle, forward, backward };
}
