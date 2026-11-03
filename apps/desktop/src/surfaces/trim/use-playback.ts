import * as React from "react";

import { clamp } from "./time";

/**
 * SOURCE OF TRUTH KEYWORDS: usePlayback, simulated playback, shuttle, JKL, play rate, playhead time
 * WHAT:  The trim window's player clock: play/pause, J/K/L shuttle rates, seeking, and
 *        stopping at the out point (or the in point when playing backwards). When the returned
 *        `media` ref is attached to a <video> (the real app), forward play runs the video and
 *        the clock follows its currentTime; reverse shuttle and seeks move its position, and
 *        `volume`/`muted` apply to it. Unattached (the browser harness) the clock is simulated.
 * WHY:   One API for both, so the timeline, readouts and keys do not change (NEW-TR-02/03).
 *        WebKit cannot play backwards, so J steps the position on each frame instead.
 * WHERE: TrimWindow.
 */
export function usePlayback({
  duration,
  inT,
  outT,
  volume = 1,
  muted = false,
}: {
  duration: number;
  inT: number;
  outT: number;
  volume?: number;
  muted?: boolean;
}) {
  const media = React.useRef<HTMLVideoElement>(null);
  const [time, setTime] = React.useState(0);
  // 0 = paused; negative plays backwards (J).
  const [rate, setRate] = React.useState(0);
  const timeRef = React.useRef(0);

  React.useEffect(() => {
    const el = media.current;
    if (el) {
      if (rate > 0) {
        el.playbackRate = rate;
        if (el.paused) el.play().catch(() => setRate(0));
      } else if (!el.paused) el.pause();
    }
    if (rate === 0) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      let next = el && rate > 0 ? el.currentTime : timeRef.current + dt * rate;
      let stop = false;
      if (rate > 0 && (next >= outT || el?.ended === true)) {
        next = outT;
        stop = true;
      } else if (rate < 0 && next <= inT) {
        next = inT;
        stop = true;
      }
      timeRef.current = next;
      setTime(next);
      if (el && (rate < 0 || stop)) el.currentTime = next;
      if (stop) setRate(0);
      else raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [rate, inT, outT]);

  React.useEffect(() => {
    const el = media.current;
    if (!el) return;
    el.volume = volume;
    el.muted = muted;
  }, [volume, muted]);

  const seek = React.useCallback(
    (t: number) => {
      const next = clamp(t, 0, duration);
      timeRef.current = next;
      setTime(next);
      const el = media.current;
      if (el) el.currentTime = next;
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

  return { media, time, rate, playing: rate !== 0, seek, play, pause, toggle, forward, backward };
}
