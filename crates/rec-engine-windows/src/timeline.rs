//! SOURCE OF TRUTH KEYWORDS: FrameClock, Pauses, constant frame rate, pacer, slot, pause cut, audio_keep, 100 ns
//! WHAT:  The recording timeline: a constant-frame-rate slot grid on the QPC clock
//!        (`FrameClock`), the pause spans cut out of it (`Pauses`), which slots the pacer
//!        writes after a wait (`slots_due`), and which part of an audio chunk survives the
//!        cuts (`audio_keep`).
//! WHY:   The file must be CFR with no gap where the user paused, and audio must land on the
//!        same timeline as the video. The rules are the macOS writer's (`writer::Pauses`):
//!        pauses are whole slots, everything after a pause shifts earlier by its length.
//!        Pure, so it is unit-tested on any OS.
//! WHERE: `win::session` (video pacer and audio writer threads).

/// Media Foundation time units per second (100 ns).
pub const MF_UNITS: i64 = 10_000_000;

/// The slot grid: slot `n` starts `n / fps` seconds after `t0` (seconds on the QPC clock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameClock {
    pub fps: u32,
    pub t0: f64,
}

impl FrameClock {
    pub fn new(fps: u32, t0: f64) -> FrameClock {
        FrameClock {
            fps: fps.max(1),
            t0,
        }
    }

    /// The slot that contains time `t` (negative before the start).
    pub fn slot_at(&self, t: f64) -> i64 {
        ((t - self.t0) * self.fps as f64).floor() as i64
    }

    /// When slot `n` starts.
    pub fn slot_start(&self, n: i64) -> f64 {
        self.t0 + n as f64 / self.fps as f64
    }

    /// Seconds from the start of the timeline to `t`, as a fractional slot.
    pub fn slot_pos(&self, t: f64) -> f64 {
        (t - self.t0) * self.fps as f64
    }
}

/// The sample time of output slot `n` in 100 ns units. Integer maths, so 60 fps never
/// drifts (slot 60 is exactly 1 s).
pub fn slot_time(n: i64, fps: u32) -> i64 {
    n * MF_UNITS / fps.max(1) as i64
}

/// The duration of output slot `n` in 100 ns units (the next slot's time minus this one's).
pub fn slot_duration(n: i64, fps: u32) -> i64 {
    slot_time(n + 1, fps) - slot_time(n, fps)
}

/// Pause spans on the real slot grid, `[pause, resume)`. Everything captured inside a span
/// is dropped and everything after it moves earlier by the span's length, so the output
/// has no gap. Same rules as the macOS writer's `Pauses`.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Pauses {
    spans: Vec<(i64, Option<i64>)>,
}

impl Pauses {
    pub fn is_paused(&self) -> bool {
        self.spans.last().is_some_and(|s| s.1.is_none())
    }

    /// Pause from `slot` on. False if already paused.
    pub fn pause(&mut self, slot: i64) -> bool {
        if self.is_paused() {
            return false;
        }
        let floor = self.spans.last().and_then(|s| s.1).unwrap_or(0);
        self.spans.push((slot.max(floor), None));
        true
    }

    /// Resume at `slot`. False if not paused.
    pub fn resume(&mut self, slot: i64) -> bool {
        let Some(last) = self.spans.last_mut() else {
            return false;
        };
        if last.1.is_some() {
            return false;
        }
        if slot <= last.0 {
            // Resumed before the pause took effect.
            self.spans.pop();
        } else {
            last.1 = Some(slot);
        }
        true
    }

    /// Slots to shift something at real slot position `slot`, or None inside a pause.
    pub fn offset_at(&self, slot: f64) -> Option<i64> {
        let mut off = 0;
        for &(p, r) in &self.spans {
            if slot < p as f64 {
                break;
            }
            match r {
                Some(r) if slot >= r as f64 => off += r - p,
                _ => return None,
            }
        }
        Some(off)
    }

    /// The longest part of `[a, b)` (real slots) outside pauses: `(from, to, offset)`.
    pub fn keep(&self, a: f64, b: f64) -> Option<(f64, f64, i64)> {
        let mut best: Option<(f64, f64, i64)> = None;
        let mut from = f64::NEG_INFINITY;
        let mut off = 0;
        let mut consider = |lo: f64, hi: f64, off: i64| {
            let (x, y) = (lo.max(a), hi.min(b));
            if y > x && best.is_none_or(|(p, q, _)| y - x > q - p) {
                best = Some((x, y, off));
            }
        };
        for &(p, r) in &self.spans {
            consider(from, p as f64, off);
            match r {
                Some(r) => {
                    off += r - p;
                    from = r as f64;
                }
                None => return best,
            }
        }
        consider(from, f64::INFINITY, off);
        best
    }

    /// Slots inside closed pauses.
    pub fn total(&self) -> i64 {
        self.spans
            .iter()
            .filter_map(|&(p, r)| r.map(|r| r - p))
            .sum()
    }
}

/// One slot the pacer handles after waking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotOut {
    /// The real slot on the grid.
    pub real: i64,
    /// Its place on the output timeline, or None while paused (nothing is written).
    pub out: Option<i64>,
}

/// The slots from `next` up to and including `now` (the slot the clock is in), each with
/// its output position. A late pacer gets several: the first carries the newest frame, the
/// rest repeat it, so the file stays constant frame rate. At most `max` are returned (the
/// rest count as dropped: a stall longer than that is not filled).
pub fn slots_due(next: i64, now: i64, pauses: &Pauses, max: usize) -> Vec<SlotOut> {
    if now < next {
        return vec![];
    }
    let first = next.max(now + 1 - max as i64);
    (first..=now)
        .map(|real| SlotOut {
            real,
            out: pauses.offset_at(real as f64).map(|off| real - off),
        })
        .collect()
}

/// Which samples of an audio chunk survive the pause cuts, and where they go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioKeep {
    /// Samples to drop from the start of the chunk.
    pub skip: usize,
    /// Samples to keep after those.
    pub take: usize,
    /// Output time of the first kept sample, in samples at `rate`.
    pub out_sample: i64,
}

/// For `frames` samples at `rate` starting `start_secs` after the timeline's start, the
/// longest run outside pauses and its output position. None when all of it is paused or it
/// lies before the start.
pub fn audio_keep(
    start_secs: f64,
    frames: usize,
    rate: f64,
    fps: u32,
    pauses: &Pauses,
) -> Option<AudioKeep> {
    if frames == 0 || rate <= 0.0 {
        return None;
    }
    let fps = fps.max(1) as f64;
    let end_secs = start_secs + frames as f64 / rate;
    // Nothing before the first video frame.
    let (a, b) = (start_secs.max(0.0) * fps, end_secs * fps);
    if b <= a {
        return None;
    }
    let (from, to, off) = pauses.keep(a, b)?;
    let skip = ((from / fps - start_secs) * rate).round().max(0.0) as usize;
    let end = ((to / fps - start_secs) * rate).round().max(0.0) as usize;
    let skip = skip.min(frames);
    let take = end.min(frames).saturating_sub(skip);
    if take == 0 {
        return None;
    }
    let out_secs = (from - off as f64) / fps;
    Some(AudioKeep {
        skip,
        take,
        out_sample: (out_secs * rate).round() as i64,
    })
}

/// Keeps one audio track's sample times continuous: a chunk that starts after the
/// previous one ended gets silence in front (up to `MAX_PAD` samples; a longer hole jumps),
/// one that overlaps it loses its first samples. AAC in MP4 has no gaps, so a hole left
/// in the timestamps would shift everything after it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrackClock {
    next: Option<i64>,
}

/// At most this much silence (1 s at 48 kHz) is inserted to close a hole.
pub const MAX_PAD: i64 = 48_000;

/// What to do with a chunk before writing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackFix {
    /// Silent samples to write first.
    pub pad: usize,
    /// Samples to drop from the chunk's start.
    pub skip: usize,
    /// Where the written run (pad + kept samples) starts.
    pub start: i64,
}

impl TrackClock {
    /// Fix for a chunk of `len` samples that starts at sample `at`; None if nothing of it
    /// is new.
    pub fn place(&mut self, at: i64, len: usize) -> Option<TrackFix> {
        let end = at + len as i64;
        let fix = match self.next {
            None => TrackFix {
                pad: 0,
                skip: 0,
                start: at,
            },
            Some(n) if at > n && at - n <= MAX_PAD => TrackFix {
                pad: (at - n) as usize,
                skip: 0,
                start: n,
            },
            Some(n) if at < n => {
                if end <= n {
                    return None;
                }
                TrackFix {
                    pad: 0,
                    skip: (n - at) as usize,
                    start: n,
                }
            }
            Some(_) => TrackFix {
                pad: 0,
                skip: 0,
                start: at,
            },
        };
        self.next = Some(end);
        Some(fix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_fps_slots_are_exact() {
        assert_eq!(slot_time(0, 60), 0);
        assert_eq!(slot_time(60, 60), MF_UNITS);
        let total: i64 = (0..60).map(|n| slot_duration(n, 60)).sum();
        assert_eq!(total, MF_UNITS, "no drift over a second");
        // 166666 or 166667 each.
        assert!((0..60).all(|n| (166_666..=166_667).contains(&slot_duration(n, 60))));
        assert_eq!(slot_time(30, 30), MF_UNITS);
        assert_eq!(slot_time(25, 25), MF_UNITS);
    }

    #[test]
    fn the_clock_maps_time_to_slots() {
        let c = FrameClock::new(30, 100.0);
        assert_eq!(c.slot_at(100.0), 0);
        assert_eq!(c.slot_at(100.0 + 1.0 / 30.0 - 1e-9), 0);
        assert_eq!(c.slot_at(101.0), 30);
        assert_eq!(c.slot_at(99.99), -1);
        assert!((c.slot_start(15) - 100.5).abs() < 1e-12);
    }

    #[test]
    fn pauses_cut_the_timeline() {
        let mut p = Pauses::default();
        assert!(p.pause(10));
        assert!(!p.pause(11), "already paused");
        assert!(p.resume(25));
        assert!(!p.resume(26), "not paused");
        assert_eq!(p.offset_at(5.0), Some(0));
        assert_eq!(p.offset_at(12.0), None);
        assert_eq!(p.offset_at(30.0), Some(15));
        assert_eq!(p.total(), 15);
        // Resuming before the pause took effect removes it.
        let mut q = Pauses::default();
        q.pause(10);
        q.resume(10);
        assert_eq!(q, Pauses::default());
    }

    #[test]
    fn a_late_pacer_repeats_the_frame_for_missed_slots() {
        let p = Pauses::default();
        assert_eq!(slots_due(5, 4, &p, 8), vec![]);
        let due = slots_due(5, 7, &p, 8);
        assert_eq!(
            due.iter().map(|s| s.out).collect::<Vec<_>>(),
            vec![Some(5), Some(6), Some(7)]
        );
        // A long stall is capped.
        assert_eq!(slots_due(0, 100, &p, 4).len(), 4);
        assert_eq!(slots_due(0, 100, &p, 4)[0].real, 97);
    }

    #[test]
    fn paused_slots_write_nothing_and_later_ones_shift() {
        let mut p = Pauses::default();
        p.pause(3);
        let due = slots_due(2, 4, &p, 8);
        assert_eq!(due[0].out, Some(2));
        assert_eq!(due[1].out, None);
        assert_eq!(due[2].out, None);
        p.resume(6);
        let due = slots_due(6, 7, &p, 8);
        assert_eq!(
            due.iter().map(|s| s.out).collect::<Vec<_>>(),
            vec![Some(3), Some(4)]
        );
    }

    #[test]
    fn tracks_stay_continuous() {
        let mut c = TrackClock::default();
        assert_eq!(
            c.place(100, 480),
            Some(TrackFix {
                pad: 0,
                skip: 0,
                start: 100
            })
        );
        // A 20-sample hole is filled.
        assert_eq!(
            c.place(600, 480),
            Some(TrackFix {
                pad: 20,
                skip: 0,
                start: 580
            })
        );
        // An overlap is trimmed.
        assert_eq!(
            c.place(1000, 480),
            Some(TrackFix {
                pad: 0,
                skip: 80,
                start: 1080
            })
        );
        // Entirely old: dropped.
        assert_eq!(c.place(0, 100), None);
        // A long hole jumps instead of writing minutes of silence.
        assert_eq!(
            c.place(1_000_000, 10),
            Some(TrackFix {
                pad: 0,
                skip: 0,
                start: 1_000_000
            })
        );
    }

    #[test]
    fn audio_follows_the_cuts() {
        let p = Pauses::default();
        // 480 samples at 48 kHz starting 1 s in, 30 fps.
        let k = audio_keep(1.0, 480, 48_000.0, 30, &p).unwrap();
        assert_eq!((k.skip, k.take, k.out_sample), (0, 480, 48_000));
        // Before the first video frame: only the part after 0 survives.
        let k = audio_keep(-0.005, 480, 48_000.0, 30, &p).unwrap();
        assert_eq!((k.skip, k.take, k.out_sample), (240, 240, 0));
        assert_eq!(audio_keep(-1.0, 480, 48_000.0, 30, &p), None);
        // Paused from slot 30 (1 s) to 60 (2 s): audio at 2.5 s lands at 1.5 s.
        let mut p = Pauses::default();
        p.pause(30);
        assert_eq!(audio_keep(1.5, 480, 48_000.0, 30, &p), None);
        p.resume(60);
        let k = audio_keep(2.5, 480, 48_000.0, 30, &p).unwrap();
        assert_eq!((k.skip, k.take, k.out_sample), (0, 480, 72_000));
        // A chunk across the pause start keeps the part before it.
        let k = audio_keep(1.0 - 0.005, 480, 48_000.0, 30, &p).unwrap();
        assert_eq!((k.skip, k.take, k.out_sample), (0, 240, 47_760));
    }
}
