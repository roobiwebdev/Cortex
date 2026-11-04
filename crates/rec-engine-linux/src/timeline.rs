//! SOURCE OF TRUTH KEYWORDS: Timeline, begin, pause, resume, end, capture time, running time, map timestamp, kept_ns, recorded_ns
//! WHAT:  The cut list of one recording, in capture time (a buffer's running time): when the
//!        recording begins, which windows were paused, when it ends; whether a buffer stamped
//!        `t` is kept, its new timestamp with everything cut moved out, and how long the file
//!        the kept buffers make is.
//! WHY:   GStreamer's live sources keep running while the user pauses, and on a busy machine a
//!        branch can reach its gate seconds after its buffers were captured. Every branch
//!        (screen, camera track, each audio track) passes a gate that asks this one timeline
//!        by the buffer's own running time, never by when it arrives, so all tracks cut the
//!        same span however late they are, and the file has no gap or frozen frame. The
//!        reported length comes from the buffers that were kept, so it is what the file holds.
//!        Pure (nanoseconds in, nanoseconds out), so it is unit tested without GStreamer.
//! WHERE: `session` (the gate pad probes, the live clock and the finished length).

/// The cuts in pipeline running time (ns). The last pause may still be open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    /// Running time the recording begins at: the file's zero. None until `begin`, and
    /// until then every buffer is dropped.
    start: Option<u64>,
    /// Paused `(start, end)`; `end` None while paused.
    spans: Vec<(u64, Option<u64>)>,
    /// File time (ns) where the latest kept buffer ends.
    kept_end: u64,
    /// A buffer was kept.
    kept: bool,
}

impl Timeline {
    /// Begin at running time `now`: buffers captured earlier (while the pipeline warmed up)
    /// are dropped, and `now` is the file's zero. False if it had begun already.
    pub fn begin(&mut self, now: u64) -> bool {
        if self.start.is_some() {
            return false;
        }
        self.start = Some(now);
        true
    }

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

    /// Nanoseconds cut from the recording between its start and running time `t`: the
    /// paused time before `t` (an open pause counts up to `t`).
    fn cut_before(&self, start: u64, t: u64) -> u64 {
        self.spans
            .iter()
            .map(|(s, e)| {
                let s = (*s).max(start);
                let e = e.unwrap_or(t).min(t);
                e.saturating_sub(s)
            })
            .sum()
    }

    /// The file timestamp of a buffer captured at running time `t`, or None to drop it
    /// (captured before the start, in a pause or after the end). Depends only on `t`, so a
    /// buffer that arrives late is treated exactly like one on time.
    pub fn map(&self, t: u64) -> Option<u64> {
        let start = self.start.filter(|s| t >= *s)?;
        let paused = self
            .spans
            .iter()
            .any(|(s, e)| t >= *s && e.is_none_or(|e| t < e));
        if paused {
            return None;
        }
        Some(t - start - self.cut_before(start, t))
    }

    /// `map` for a buffer the gate passes on: also notes where it ends in the file
    /// (`duration` ns after its timestamp), for `kept_ns`.
    pub fn keep(&mut self, t: u64, duration: u64) -> Option<u64> {
        let mapped = self.map(t)?;
        self.kept = true;
        self.kept_end = self.kept_end.max(mapped.saturating_add(duration));
        Some(mapped)
    }

    /// A buffer has been kept: the file has something in it.
    pub fn has_started(&self) -> bool {
        self.kept
    }

    /// Length of the file the kept buffers make (ns): where the latest one ends. This is
    /// the recording's length, whatever the clock said, when a branch could not catch up.
    pub fn kept_ns(&self) -> u64 {
        self.kept_end
    }

    /// Recorded nanoseconds at running time `now`, the live clock: the time since the start
    /// minus every pause (an open pause counts up to `now`).
    pub fn recorded_ns(&self, now: u64) -> u64 {
        let Some(start) = self.start else {
            return 0;
        };
        now.saturating_sub(start)
            .saturating_sub(self.cut_before(start, now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1_000_000_000;
    const MS: u64 = 1_000_000;

    fn begun(at: u64) -> Timeline {
        let mut t = Timeline::default();
        assert!(t.begin(at));
        t
    }

    #[test]
    fn nothing_paused_keeps_every_timestamp() {
        let t = begun(0);
        assert_eq!(t.map(0), Some(0));
        assert_eq!(t.map(5 * S), Some(5 * S));
    }

    #[test]
    fn nothing_is_kept_before_the_start_and_the_start_is_zero() {
        let mut t = Timeline::default();
        assert_eq!(t.map(S), None, "not begun: the pipeline is warming up");
        assert_eq!(t.recorded_ns(S), 0);
        assert!(t.begin(2 * S));
        assert!(!t.begin(3 * S), "begins once");
        // A frame captured during warm-up that reaches its gate after the start is dropped.
        assert_eq!(t.map(2 * S - 1), None);
        assert_eq!(t.map(2 * S), Some(0));
        assert_eq!(t.map(3 * S), Some(S));
        assert_eq!(t.recorded_ns(5 * S), 3 * S);
    }

    #[test]
    fn a_pause_drops_its_span_and_shifts_what_follows() {
        let mut t = begun(0);
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
        let mut t = begun(0);
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
    fn late_buffers_are_cut_by_when_they_were_captured() {
        // The user pauses at 2 s and resumes at 3.5 s. The audio is on time; the picture
        // reaches its gate 1.2 s after capture.
        let mut t = begun(0);
        t.pause(2 * S);
        // Arriving during the pause, captured before it: kept, timestamps untouched.
        assert_eq!(t.keep(800 * MS, 33 * MS), Some(800 * MS));
        assert_eq!(t.keep(1999 * MS, 33 * MS), Some(1999 * MS));
        t.resume(3500 * MS);
        // Arriving after the resume, captured during the pause: dropped.
        assert_eq!(t.keep(2300 * MS, 33 * MS), None);
        assert_eq!(t.keep(3499 * MS, 33 * MS), None);
        // Captured after the resume: moved back by the pause.
        assert_eq!(t.keep(3500 * MS, 33 * MS), Some(2 * S));
        // Every track gets the same timestamp for the same capture time, in any order.
        let mut audio = t.clone();
        assert_eq!(audio.keep(4 * S, 10 * MS), t.keep(4 * S, 33 * MS));
    }

    #[test]
    fn a_branch_far_behind_still_cuts_the_same_span() {
        // The picture runs 3 s behind: by the time the user resumes it has not yet reached
        // the pause, and it reaches the stop long after it.
        let mut t = begun(0);
        t.pause(2 * S);
        t.resume(3500 * MS);
        let end = t.end(5500 * MS);
        assert_eq!(end, 5500 * MS);
        // Frames every 100 ms, reaching the gate in any order: only capture time counts.
        let kept: Vec<u64> = (0..100)
            .rev()
            .map(|i| i * 100 * MS)
            .filter_map(|rt| t.keep(rt, 100 * MS))
            .collect();
        // 2 s before the pause and 2 s after it, nothing from the pause or past the stop.
        assert_eq!(kept.len(), 20 + 20);
        assert!(kept.iter().all(|m| *m < 4 * S));
        assert_eq!(t.kept_ns(), 4 * S);
    }

    #[test]
    fn the_end_drops_what_follows_and_freezes_the_clock() {
        let mut t = begun(0);
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
        let mut t = begun(0);
        t.pause(2 * S);
        assert_eq!(t.end(5 * S), 2 * S);
        assert_eq!(t.map(3 * S), None);
        assert_eq!(t.recorded_ns(5 * S), 2 * S);
    }

    #[test]
    fn recorded_time_excludes_pauses() {
        let mut t = Timeline::default();
        assert_eq!(t.recorded_ns(5 * S), 0, "not begun");
        t.begin(S);
        assert_eq!(t.recorded_ns(3 * S), 2 * S);
        t.pause(3 * S);
        assert_eq!(t.recorded_ns(4 * S), 2 * S, "frozen while paused");
        t.resume(6 * S);
        assert_eq!(t.recorded_ns(7 * S), 3 * S);
    }

    #[test]
    fn the_length_is_what_was_kept_not_what_the_clock_says() {
        let mut t = begun(0);
        assert!(!t.has_started());
        assert_eq!(t.kept_ns(), 0);
        t.pause(2 * S);
        t.resume(3 * S);
        t.end(5 * S);
        // Every branch reached the stop: the length is the clock's, 4 s.
        for i in 0..500 {
            t.keep(i * 10 * MS, 10 * MS);
        }
        assert!(t.has_started());
        assert_eq!(t.kept_ns(), 4 * S);
        assert_eq!(t.recorded_ns(5 * S), 4 * S);

        // No branch got past 4.5 s (3.5 s of file) before the file was closed: the length
        // ends there too, though the clock says 4 s.
        let mut t = begun(0);
        t.pause(2 * S);
        t.resume(3 * S);
        t.end(5 * S);
        for i in 0..450 {
            t.keep(i * 10 * MS, 10 * MS);
        }
        assert_eq!(t.kept_ns(), 3500 * MS);
        assert_eq!(t.recorded_ns(5 * S), 4 * S);
    }
}
