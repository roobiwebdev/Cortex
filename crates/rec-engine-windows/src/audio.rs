//! SOURCE OF TRUTH KEYWORDS: Mixer, Resampler, Fader, Volume, Limiter, SampleFormat, to_stereo, audio tracks, 48 kHz
//! WHAT:  Audio maths for the Windows engine: device samples (any PCM format, any channel
//!        count) to 48 kHz stereo f32, the fader with click-free ramps, a streaming sinc
//!        resampler, the mix bus limiter, a timeline mixer for system audio + mic, and the
//!        track layout (mix, system, mic).
//! WHY:   WASAPI hands over whatever the device's mix format is (often 48 kHz float, but a
//!        44.1 kHz USB mic or a 16 kHz headset is common). The rules and constants are the
//!        macOS engine's (`rec-engine-macos/src/mixer.rs`), ported without CoreMedia, so a
//!        recording sounds the same on both. Pure, so it is unit-tested on any OS.
//! WHERE: `win::audio` (capture threads) and `win::session` (audio writer thread).

use std::collections::VecDeque;

use rec_core::settings::{AudioTracks, GAIN_DB_RANGE};

/// The recording's sample rate.
pub const RATE: f64 = 48_000.0;
/// Emit the mix even if a source is silent or stalled for longer than this (0.5 s).
const MAX_WAIT: i64 = 24_000;
/// Timestamp jumps up to this many samples count as continuous (2 ms).
const SNAP: i64 = 96;

/// A source's fader: gain in dB and mute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub db: f32,
    pub muted: bool,
}

impl Volume {
    pub const UNITY: Volume = Volume {
        db: 0.0,
        muted: false,
    };

    /// Linear gain of the fader alone: silence at the bottom of its travel.
    pub fn fader_gain(self) -> f32 {
        if self.db > GAIN_DB_RANGE.0 {
            10f32.powf(self.db.min(GAIN_DB_RANGE.1) / 20.0)
        } else {
            0.0
        }
    }

    /// Linear gain applied to the source's audio.
    pub fn gain(self) -> f32 {
        if self.muted { 0.0 } else { self.fader_gain() }
    }
}

/// How long a gain change takes.
const RAMP_SECS: f32 = 0.02;

/// Applies a source's gain; a new gain is reached over `RAMP_SECS`, so faders and mutes
/// never click.
#[derive(Clone, Debug)]
pub struct Fader {
    gain: f32,
    target: f32,
    span: f32,
}

impl Fader {
    pub fn new(gain: f32) -> Self {
        Fader {
            gain,
            target: gain,
            span: 0.0,
        }
    }

    pub fn set(&mut self, gain: f32) {
        if gain != self.target {
            self.target = gain;
            self.span = (gain - self.gain).abs();
        }
    }

    /// Apply the gain to `frames` in place (at `RATE`).
    pub fn apply(&mut self, frames: &mut [[f32; 2]]) {
        if self.gain == 1.0 && self.target == 1.0 {
            return;
        }
        let step = (self.span / (RAMP_SECS * RATE as f32)).max(1e-5);
        for fr in frames {
            let d = self.target - self.gain;
            self.gain = if d.abs() <= step {
                self.target
            } else {
                self.gain + step.copysign(d)
            };
            fr[0] *= self.gain;
            fr[1] *= self.gain;
        }
    }
}

/// How a device delivers samples (`WAVEFORMATEX` / `WAVEFORMATEXTENSIBLE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    F32,
    I16,
    I24,
    /// 24 bits of signal in a 32-bit container (`wValidBitsPerSample` 24).
    I24In32,
    I32,
}

impl SampleFormat {
    /// From the format tag's float flag, the container size and the valid bits.
    pub fn from_wave(float: bool, bits: u16, valid_bits: u16) -> Option<SampleFormat> {
        match (float, bits, valid_bits) {
            (true, 32, _) => Some(SampleFormat::F32),
            (false, 16, _) => Some(SampleFormat::I16),
            (false, 24, _) => Some(SampleFormat::I24),
            (false, 32, 24) => Some(SampleFormat::I24In32),
            (false, 32, _) => Some(SampleFormat::I32),
            _ => None,
        }
    }

    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::I16 => 2,
            SampleFormat::I24 => 3,
            SampleFormat::F32 | SampleFormat::I24In32 | SampleFormat::I32 => 4,
        }
    }

    fn read(self, b: &[u8]) -> f32 {
        match self {
            SampleFormat::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            SampleFormat::I16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            SampleFormat::I24 => {
                let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                v as f32 / 8_388_608.0
            }
            SampleFormat::I24In32 | SampleFormat::I32 => {
                i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0
            }
        }
    }
}

/// Interleaved device samples to stereo frames: mono is copied to both sides, more than
/// two channels keep the front left and right (the first two, in WASAPI's channel order).
pub fn to_stereo(data: &[u8], format: SampleFormat, channels: usize) -> Vec<[f32; 2]> {
    let ch = channels.max(1);
    let frame_bytes = ch * format.bytes();
    data.chunks_exact(frame_bytes)
        .map(|f| {
            let l = format.read(f);
            let r = if ch > 1 {
                format.read(&f[format.bytes()..])
            } else {
                l
            };
            let clean = |v: f32| if v.is_finite() { v } else { 0.0 };
            [clean(l), clean(r)]
        })
        .collect()
}

/// Peak level (0..1, linear) of some frames.
pub fn peak(frames: &[[f32; 2]]) -> f32 {
    frames
        .iter()
        .flat_map(|f| f.iter())
        .fold(0.0f32, |m, s| m.max(s.abs()))
        .min(1.0)
}

/// Stereo frames to interleaved 16-bit PCM bytes (what the AAC encoder takes).
pub fn to_pcm16(frames: &[[f32; 2]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frames.len() * 4);
    for fr in frames {
        for s in fr {
            let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Taps on each side of a resampled output sample.
const HALF_TAPS: usize = 24;

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term) = (1.0, 1.0);
    for k in 1..32 {
        term *= (x / (2.0 * k as f64)).powi(2);
        sum += term;
    }
    sum
}

/// Streaming Kaiser-windowed sinc resampler to 48 kHz for one source, with its history
/// carried across buffers so boundaries are seamless (the macOS engine's resampler).
#[derive(Clone, Debug)]
pub struct Resampler {
    from: f64,
    hist: Vec<[f32; 2]>,
    first: i64,
    pos: f64,
    t0: f64,
    next_in: i64,
}

impl Resampler {
    pub fn new(from: f64, t0: f64) -> Self {
        Resampler {
            from,
            hist: vec![],
            first: 0,
            pos: 0.0,
            t0,
            next_in: 0,
        }
    }

    /// Resample a buffer that starts at `pts` seconds. Returns the time of the first output
    /// frame and the frames. A buffer that doesn't continue the stream restarts it.
    pub fn push(&mut self, pts: f64, rate: f64, src: Vec<[f32; 2]>) -> (f64, Vec<[f32; 2]>) {
        let expected = self.t0 + self.next_in as f64 / self.from;
        if rate != self.from || (pts - expected).abs() > SNAP as f64 / RATE {
            *self = Resampler::new(rate, pts);
        }
        self.next_in += src.len() as i64;
        self.hist.extend(src);
        let step = self.from / RATE;
        let fc = 0.5 * (RATE / self.from).min(1.0) * 0.94;
        let beta = 8.0;
        let norm = bessel_i0(beta);
        let mut out = vec![];
        let t_out = self.t0 + self.pos / self.from;
        let h = HALF_TAPS as f64 / (2.0 * fc).min(1.0);
        let reach = h.ceil() as i64;
        while (self.pos.floor() as i64) + reach < self.first + self.hist.len() as i64 {
            let c = self.pos.floor() as i64;
            let (mut l, mut r, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
            for j in (c - reach + 1)..=(c + reach) {
                let x = j as f64 - self.pos;
                if x.abs() >= h {
                    continue;
                }
                let arg = 2.0 * fc * x;
                let sinc = if arg.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * arg).sin() / (std::f64::consts::PI * arg)
                };
                let w = bessel_i0(beta * (1.0 - (x / h).powi(2)).max(0.0).sqrt()) / norm;
                let k = 2.0 * fc * sinc * w;
                wsum += k;
                if j >= self.first
                    && let Some(s) = self.hist.get((j - self.first) as usize)
                {
                    l += k * s[0] as f64;
                    r += k * s[1] as f64;
                }
            }
            let g = if wsum > 0.0 { 1.0 / wsum } else { 0.0 };
            out.push([(l * g) as f32, (r * g) as f32]);
            self.pos += step;
        }
        let keep_from = (self.pos.floor() as i64 - reach).max(self.first);
        let drop = (keep_from - self.first) as usize;
        self.hist.drain(..drop.min(self.hist.len()));
        self.first += drop as i64;
        (t_out, out)
    }
}

/// -1 dBFS: headroom for the AAC encoder's overshoot.
const CEILING: f32 = 0.891;
const ATTACK: f32 = 0.35;
const RELEASE: f32 = 1.0 / (0.15 * RATE as f32);

/// Mix bus limiter: fast attack, slow release, soft knee.
#[derive(Clone, Debug)]
struct Limiter {
    gain: f32,
}

impl Limiter {
    fn process(&mut self, fr: [f32; 2]) -> [f32; 2] {
        let peak = fr[0].abs().max(fr[1].abs());
        let want = if peak > CEILING { CEILING / peak } else { 1.0 };
        self.gain = if want < self.gain {
            self.gain + (want - self.gain) * ATTACK
        } else {
            (self.gain + RELEASE).min(want)
        };
        fr.map(|s| soft_clip(s * self.gain))
    }
}

fn soft_clip(s: f32) -> f32 {
    const KNEE: f32 = 0.9;
    let a = s.abs();
    if a <= KNEE {
        return s;
    }
    let room = 1.0 - KNEE;
    (KNEE + room * ((a - KNEE) / room).tanh()).copysign(s)
}

/// Which source a buffer came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    System,
    Mic,
}

impl Source {
    pub fn index(self) -> usize {
        match self {
            Source::System => 0,
            Source::Mic => 1,
        }
    }
}

/// Places each source's 48 kHz frames on one sample timeline from their capture times and
/// sums them, so the mix stays in sync with video. A source that stalls (WASAPI loopback
/// sends nothing while nothing plays) is filled with silence after `MAX_WAIT`.
#[derive(Clone, Debug)]
pub struct Mixer {
    t0: i64,
    base: i64,
    buf: VecDeque<[f32; 2]>,
    horizon: [Option<i64>; 2],
    active: [bool; 2],
    resamplers: [Option<Resampler>; 2],
    limiter: Option<Limiter>,
}

impl Mixer {
    /// `t0_secs`: the timeline's start on the capture clock. `limit`: run the mix bus
    /// limiter (the mix track; single-source stems pass through like on macOS).
    pub fn new(t0_secs: f64, system: bool, mic: bool, limit: bool) -> Self {
        Mixer {
            t0: (t0_secs * RATE).round() as i64,
            base: 0,
            buf: VecDeque::new(),
            horizon: [None, None],
            active: [system, mic],
            resamplers: [None, None],
            limiter: limit.then_some(Limiter { gain: 1.0 }),
        }
    }

    pub fn is_active(&self, s: Source) -> bool {
        self.active[s.index()]
    }

    /// Add frames that start at `pts` seconds on the capture clock, at `rate`.
    pub fn push(&mut self, s: Source, pts: f64, rate: f64, frames: Vec<[f32; 2]>) {
        let i = s.index();
        if !self.active[i] {
            return;
        }
        let (pts, frames) = if (rate - RATE).abs() < 0.5 || rate <= 0.0 {
            self.resamplers[i] = None;
            (pts, frames)
        } else {
            self.resamplers[i]
                .get_or_insert_with(|| Resampler::new(rate, pts))
                .push(pts, rate, frames)
        };
        if frames.is_empty() {
            return;
        }
        let mut start = (pts * RATE).round() as i64 - self.t0;
        if let Some(h) = self.horizon[i]
            && (start - h).abs() <= SNAP
        {
            start = h;
        }
        let end = start + frames.len() as i64;
        self.horizon[i] = Some(self.horizon[i].map_or(end, |h| h.max(end)));
        let need = (end - self.base).max(0) as usize;
        if self.buf.len() < need {
            self.buf.resize(need, [0.0, 0.0]);
        }
        for (n, fr) in frames.iter().enumerate() {
            let pos = start + n as i64 - self.base;
            if pos < 0 {
                continue;
            }
            if let Some(slot) = self.buf.get_mut(pos as usize) {
                slot[0] += fr[0];
                slot[1] += fr[1];
            }
        }
    }

    /// Frames every active source has reached (or anything `MAX_WAIT` behind the newest;
    /// everything with `flush`), with their start in seconds after `t0`.
    pub fn pull(&mut self, flush: bool) -> Option<(f64, Vec<[f32; 2]>)> {
        let hs: Vec<i64> = (0..2)
            .filter(|&i| self.active[i])
            .map(|i| self.horizon[i].unwrap_or(0))
            .collect();
        let max_h = hs.iter().copied().max().unwrap_or(0);
        let until = if flush {
            max_h
        } else {
            hs.iter().copied().min().unwrap_or(0).max(max_h - MAX_WAIT)
        };
        let n = (until - self.base).min(self.buf.len() as i64);
        if n <= 0 {
            return None;
        }
        let t = self.base as f64 / RATE;
        self.base += n;
        let mut frames: Vec<[f32; 2]> = self.buf.drain(..n as usize).collect();
        if let Some(l) = &mut self.limiter {
            for fr in &mut frames {
                *fr = l.process(*fr);
            }
        }
        Some((t, frames))
    }
}

/// Silence to stand in for a source that sent nothing (WASAPI loopback is quiet while
/// nothing plays): when its last buffer ended at `last_end` and it is now `now` (seconds),
/// the start and frame count of the silence up to `now - latency`, once the hole is longer
/// than `latency`. None otherwise.
pub fn silence_gap(last_end: f64, now: f64, latency: f64) -> Option<(f64, usize)> {
    let until = now - latency;
    if until - last_end < latency {
        return None;
    }
    let frames = ((until - last_end) * RATE).floor() as usize;
    (frames > 0).then_some((last_end, frames))
}

/// One audio track of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackRole {
    /// System audio + mic (what players play).
    Mix,
    System,
    Mic,
}

/// The audio tracks a recording gets, in file order: the macOS writer's layout. With both
/// sources, `Both` = mix, system, mic; `Mix` = the mix only; `Stems` = system, mic. With
/// one source, that source alone; with none, no track.
pub fn track_layout(system: bool, mic: bool, layout: AudioTracks) -> Vec<TrackRole> {
    match (system, mic) {
        (true, true) => match layout {
            AudioTracks::Both => vec![TrackRole::Mix, TrackRole::System, TrackRole::Mic],
            AudioTracks::Mix => vec![TrackRole::Mix],
            AudioTracks::Stems => vec![TrackRole::System, TrackRole::Mic],
        },
        (true, false) => vec![TrackRole::System],
        (false, true) => vec![TrackRole::Mic],
        (false, false) => vec![],
    }
}

/// Which sources feed a track.
pub fn track_sources(role: TrackRole) -> (bool, bool) {
    match role {
        TrackRole::Mix => (true, true),
        TrackRole::System => (true, false),
        TrackRole::Mic => (false, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faders_follow_the_settings_range() {
        assert_eq!(Volume::UNITY.gain(), 1.0);
        assert_eq!(
            Volume {
                db: -60.0,
                muted: false
            }
            .gain(),
            0.0
        );
        assert!(
            (Volume {
                db: 6.0,
                muted: false
            }
            .gain()
                - 1.995)
                .abs()
                < 0.01
        );
        let muted = Volume {
            db: 6.0,
            muted: true,
        };
        assert_eq!(muted.gain(), 0.0);
        assert!(muted.fader_gain() > 1.9, "meters still see a muted source");
    }

    #[test]
    fn a_fader_ramps_instead_of_jumping() {
        let mut f = Fader::new(1.0);
        let mut x = vec![[1.0, 1.0]; 2000];
        f.apply(&mut x);
        assert!(x.iter().all(|s| s[0] == 1.0), "unity passes through");
        f.set(0.0);
        let mut x = vec![[1.0, 1.0]; 2000];
        f.apply(&mut x);
        assert!(x[0][0] > 0.99 && x[0][0] < 1.0);
        // 20 ms at 48 kHz = 960 frames to reach silence.
        assert!(x[900][0] > 0.0);
        assert_eq!(x[1000][0], 0.0);
    }

    #[test]
    fn device_formats_become_stereo() {
        let f32s: Vec<u8> = [0.5f32, -0.25, 1.0, 0.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            to_stereo(&f32s, SampleFormat::F32, 2),
            vec![[0.5, -0.25], [1.0, 0.0]]
        );
        // Mono to both sides.
        let i16s: Vec<u8> = [16_384i16, -32_768]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            to_stereo(&i16s, SampleFormat::I16, 1),
            vec![[0.5, 0.5], [-1.0, -1.0]]
        );
        // 24-bit packed, negative.
        let i24 = [0x00, 0x00, 0xC0, 0x00, 0x00, 0x40];
        assert_eq!(to_stereo(&i24, SampleFormat::I24, 2), vec![[-0.5, 0.5]]);
        // 5.1: front left and right.
        let six: Vec<u8> = [0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(to_stereo(&six, SampleFormat::F32, 6), vec![[0.1, 0.2]]);
        assert_eq!(
            SampleFormat::from_wave(false, 32, 24),
            Some(SampleFormat::I24In32)
        );
        assert_eq!(SampleFormat::from_wave(true, 64, 64), None);
    }

    #[test]
    fn pcm16_saturates() {
        assert_eq!(to_pcm16(&[[2.0, -2.0]]), [0xFF, 0x7F, 0x01, 0x80]);
        assert_eq!(peak(&[[0.2, -0.7], [0.1, 0.0]]), 0.7);
    }

    #[test]
    fn resampling_44k1_keeps_duration_and_level() {
        let mut r = Resampler::new(44_100.0, 0.0);
        // One second of a 1 kHz sine at 44.1 kHz in 10 ms buffers.
        let mut out = vec![];
        for b in 0..100 {
            let src: Vec<[f32; 2]> = (0..441)
                .map(|i| {
                    let t = (b * 441 + i) as f64 / 44_100.0;
                    let v = (2.0 * std::f64::consts::PI * 1000.0 * t).sin() as f32 * 0.5;
                    [v, v]
                })
                .collect();
            let (_, o) = r.push(b as f64 * 0.01, 44_100.0, src);
            out.extend(o);
        }
        // About 48000 out, minus the filter's look-ahead.
        assert!((47_900..=48_000).contains(&out.len()), "{}", out.len());
        let p = peak(&out[1000..]);
        assert!((p - 0.5).abs() < 0.01, "{p}");
    }

    #[test]
    fn the_mixer_sums_sources_on_one_timeline() {
        let mut m = Mixer::new(10.0, true, true, false);
        m.push(Source::System, 10.0, RATE, vec![[0.25, 0.25]; 480]);
        // The mic hasn't arrived: nothing is ready yet.
        assert_eq!(m.pull(false), None);
        m.push(Source::Mic, 10.0, RATE, vec![[0.5, 0.5]; 480]);
        let (t, f) = m.pull(false).unwrap();
        assert_eq!((t, f.len()), (0.0, 480));
        assert_eq!(f[0], [0.75, 0.75]);
        // Sub-2 ms jitter snaps to continuous.
        m.push(
            Source::System,
            10.0 + 480.0 / RATE + 0.0005,
            RATE,
            vec![[0.1, 0.1]; 480],
        );
        m.push(
            Source::Mic,
            10.0 + 480.0 / RATE,
            RATE,
            vec![[0.0, 0.0]; 480],
        );
        let (t, f) = m.pull(false).unwrap();
        assert_eq!((t, f.len()), (0.01, 480));
        assert_eq!(f[0], [0.1, 0.1]);
    }

    #[test]
    fn a_silent_loopback_does_not_hold_the_mic_back() {
        let mut m = Mixer::new(0.0, true, true, false);
        // The system source sends nothing (nothing plays); 0.6 s of mic arrives.
        for i in 0..60 {
            m.push(Source::Mic, i as f64 * 0.01, RATE, vec![[0.1, 0.1]; 480]);
        }
        let (_, f) = m.pull(false).unwrap();
        assert_eq!(f.len(), 60 * 480 - MAX_WAIT as usize);
        let (_, rest) = m.pull(true).unwrap();
        assert_eq!(f.len() + rest.len(), 60 * 480);
    }

    #[test]
    fn the_limiter_keeps_the_mix_under_full_scale() {
        let mut m = Mixer::new(0.0, true, true, true);
        m.push(Source::System, 0.0, RATE, vec![[0.9, 0.9]; 4800]);
        m.push(Source::Mic, 0.0, RATE, vec![[0.9, 0.9]; 4800]);
        let (_, f) = m.pull(true).unwrap();
        assert!(f.iter().all(|s| s[0] <= 1.0));
        assert!(f[4000][0] < 0.95);
    }

    #[test]
    fn a_quiet_loopback_is_filled_with_silence() {
        assert_eq!(silence_gap(10.0, 10.15, 0.1), None);
        let (t, n) = silence_gap(10.0, 10.3, 0.1).unwrap();
        assert_eq!(t, 10.0);
        assert_eq!(n, 9_600);
    }

    #[test]
    fn tracks_follow_the_mac_layout() {
        use TrackRole::*;
        assert_eq!(
            track_layout(true, true, AudioTracks::Both),
            vec![Mix, System, Mic]
        );
        assert_eq!(track_layout(true, true, AudioTracks::Mix), vec![Mix]);
        assert_eq!(
            track_layout(true, true, AudioTracks::Stems),
            vec![System, Mic]
        );
        assert_eq!(track_layout(false, true, AudioTracks::Both), vec![Mic]);
        assert_eq!(track_layout(false, false, AudioTracks::Both), vec![]);
        assert_eq!(track_sources(Mix), (true, true));
    }
}
