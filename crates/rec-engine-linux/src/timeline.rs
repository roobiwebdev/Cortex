//! SOURCE OF TRUTH KEYWORDS: Timeline, pause, resume, cut the timeline, running time, map timestamp, recorded_ns
//! WHAT:  The pause bookkeeping of one recording: which running-time windows were paused,
//!        whether a buffer stamped `t` is kept, and its new timestamp with the pauses cut out.
//! WHY:   GStreamer's live sources keep running while the user pauses. Every branch (screen,
//!        camera track, each audio track) passes a gate that asks this one timeline, so all
//!        tracks drop the same span and stay in sync, and the file has no gap or frozen frame.
//!        Pure (nanoseconds in, nanoseconds out), so it is unit tested without GStreamer.
//! WHERE: `session` (the gate pad probes and the recorded clock).

/// Paused spans in pipeline running time (ns). The last one may still be open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    /// `(start, end)`; `end` None while paused.
    spans: Vec<(u64, Option<u64>)>,
    /// Running time of the first kept buffer: the file's zero.
    origin: Option<u64>,
}

impl Timeline {
    pub fn is_paused(&self) -> bool {
        self.spans.last().is_some_and(|(_, end)| end.is_none())
    }

    /// Pause at running time `now`. False if already paused.
    pub fn pause(&mut self, now: u64) -> bool {
        if self.is_paused() {
            return false;
        }
        self.spans.push((now, None));
        true
    }

    /// Resume at running time `now`. False if not paused.
    pub fn resume(&mut self, now: u64) -> bool {
        match self.spans.last_mut() {
            Some((start, end @ None)) => {
                *end = Some(now.max(*start));
                true
            }
            _ => false,
        }
    }

    /// End the recording at running time `now`: from then on every buffer is dropped (a
    /// pause in progress already drops them and stays open). Returns the running time the
    /// file ends at, which each gate must pass before EOS.
    pub fn end(&mut self, now: u64) -> u64 {
        self.pause(now);
        self.spans.last().map_or(now, |(start, _)| *start)
    }

    /// Paused nanoseconds before running time `t` (closed spans that ended by then).
    fn cut_before(&self, t: u64) -> u64 {
        self.spans
            .iter()
            .filter_map(|(s, e)| e.filter(|e| *e <= t).map(|e| e - s))
            .sum()
    }

    /// The timestamp a buffer stamped `t` gets, or None to drop it (it falls in a pause).
    pub fn map(&self, t: u64) -> Option<u64> {
        let paused = self
            .spans
            .iter()
            .any(|(s, e)| t >= *s && e.is_none_or(|e| t < e));
        if paused {
            return None;
        }
        Some(t - self.cut_before(t))
    }

    /// Remember the first kept buffer's running time (the gates call this for every buffer;
    /// only the first one counts).
    pub fn mark_origin(&mut self, t: u64) {
        if self.origin.is_none() {
            self.origin = Some(t);
        }
    }

    pub fn has_started(&self) -> bool {
        self.origin.is_some()
    }

    /// Recorded nanoseconds at running time `now`: the time since the first buffer minus
    /// every pause (an open pause counts up to `now`).
    pub fn recorded_ns(&self, now: u64) -> u64 {
        let Some(origin) = self.origin else {
            return 0;
        };
        let paused: u64 = self
            .spans
            .iter()
            .map(|(s, e)| {
                let s = (*s).max(origin);
                let e = e.unwrap_or(now).min(now);
                e.saturating_sub(s)
            })
            .sum();
        now.saturating_sub(origin).saturating_sub(paused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1_000_000_000;

    #[test]
    fn nothing_paused_keeps_every_timestamp() {
        let t = Timeline::default();
        assert_eq!(t.map(0), Some(0));
        assert_eq!(t.map(5 * S), Some(5 * S));
    }

    #[test]
    fn a_pause_drops_its_span_and_shifts_what_follows() {
        let mut t = Timeline::default();
        assert!(t.pause(2 * S));
        assert!(!t.pause(3 * S), "already paused");
        assert_eq!(t.map(S), Some(S));
        assert_eq!(t.map(2 * S), None);
        assert_eq!(t.map(10 * S), None, "open pause drops everything after it");
        assert!(t.resume(5 * S));
        assert!(!t.resume(6 * S), "not paused");
        assert_eq!(t.map(4 * S), None);
        assert_eq!(t.map(5 * S), Some(2 * S));
        assert_eq!(t.map(7 * S), Some(4 * S));
    }

    #[test]
    fn pauses_add_up() {
        let mut t = Timeline::default();
        t.pause(S);
        t.resume(2 * S);
        t.pause(4 * S);
        t.resume(7 * S);
        assert_eq!(t.map(3 * S), Some(2 * S));
        assert_eq!(t.map(8 * S), Some(4 * S));
        // Timestamps stay monotonic across the cut.
        let mapped: Vec<_> = (0..90).filter_map(|i| t.map(i * S / 10)).collect();
        assert!(mapped.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_end_drops_what_follows_and_freezes_the_clock() {
        let mut t = Timeline::default();
        t.mark_origin(0);
        t.pause(S);
        t.resume(2 * S);
        assert_eq!(t.end(4 * S), 4 * S);
        assert_eq!(
            t.map(3 * S),
            Some(2 * S),
            "late buffers from before the stop kept"
        );
        assert_eq!(t.map(4 * S), None);
        assert_eq!(t.recorded_ns(9 * S), 3 * S);

        // Stopping while paused ends the file where the pause began.
        let mut t = Timeline::default();
        t.mark_origin(0);
        t.pause(2 * S);
        assert_eq!(t.end(5 * S), 2 * S);
        assert_eq!(t.map(3 * S), None);
        assert_eq!(t.recorded_ns(5 * S), 2 * S);
    }

    #[test]
    fn recorded_time_excludes_pauses() {
        let mut t = Timeline::default();
        assert_eq!(t.recorded_ns(5 * S), 0, "no buffer yet");
        t.mark_origin(S);
        t.mark_origin(2 * S);
        assert_eq!(t.recorded_ns(3 * S), 2 * S);
        t.pause(3 * S);
        assert_eq!(t.recorded_ns(4 * S), 2 * S, "frozen while paused");
        t.resume(6 * S);
        assert_eq!(t.recorded_ns(7 * S), 3 * S);
    }
}
