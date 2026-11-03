//! SOURCE OF TRUTH KEYWORDS: test pattern, pattern_frame, pattern_tone, flash, beep, A/V sync check, self-test
//! WHAT:  A synthetic picture and sound for self-test recordings: a moving bar over a
//!        gradient with a white flash on every whole second, and a 1 kHz beep for the first
//!        100 ms of every second.
//! WHY:   `CORTEX_DEBUG_TEST_PATTERN=1` must record without a screen or devices (CI, the
//!        user's Windows PC over SSH), and flash and beep line up, so A/V sync can be read
//!        off the file with ffprobe, as with Rec's macOS test pattern.
//! WHERE: `win::session` when `EngineOptions::test_pattern` is set.

/// BGRA pixels for frame `n` of a `w`x`h` pattern at `fps`.
pub fn pattern_frame(n: u64, fps: u32, w: u32, h: u32) -> Vec<u8> {
    let fps = fps.max(1) as u64;
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];
    if n.is_multiple_of(fps) {
        out.fill(255);
        return out;
    }
    let bar_w = (w / 40).max(2);
    let bar_x = ((n * 8) % w.max(1) as u64) as u32;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let on_bar = x >= bar_x && x < bar_x + bar_w;
            let (b, g, r) = if on_bar {
                (255, 255, 255)
            } else {
                (
                    (x * 255 / w.max(1)) as u8,
                    (y * 255 / h.max(1)) as u8,
                    ((n % fps) * 255 / fps) as u8,
                )
            };
            out[i] = b;
            out[i + 1] = g;
            out[i + 2] = r;
            out[i + 3] = 255;
        }
    }
    out
}

/// `count` stereo frames of the pattern's sound starting at sample `start` (48 kHz).
pub fn pattern_tone(start: u64, count: usize) -> Vec<[f32; 2]> {
    const RATE: u64 = 48_000;
    (0..count as u64)
        .map(|i| {
            let s = start + i;
            if s % RATE < RATE / 10 {
                let t = s as f64 / RATE as f64;
                let v = (2.0 * std::f64::consts::PI * 1000.0 * t).sin() as f32 * 0.5;
                [v, v]
            } else {
                [0.0, 0.0]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_seconds_flash_white() {
        let f = pattern_frame(60, 60, 8, 4);
        assert!(f.iter().all(|b| *b == 255));
        let f = pattern_frame(61, 60, 80, 4);
        assert_eq!(f.len(), 80 * 4 * 4);
        assert!(f.iter().any(|b| *b != 255));
        assert!(f.chunks(4).all(|p| p[3] == 255), "opaque");
    }

    #[test]
    fn the_beep_lines_up_with_the_flash() {
        let a = pattern_tone(48_000, 4_800);
        assert!(a.iter().any(|s| s[0].abs() > 0.4));
        let quiet = pattern_tone(48_000 + 4_800, 1000);
        assert!(quiet.iter().all(|s| s[0] == 0.0));
    }
}
