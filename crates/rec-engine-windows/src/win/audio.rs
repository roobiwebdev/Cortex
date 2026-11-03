//! SOURCE OF TRUTH KEYWORDS: AudioCapture, WASAPI, loopback, IAudioClient, IAudioCaptureClient, AUTOCONVERTPCM, mix format, QPC position, microphones, list_mics
//! WHAT:  Audio sources for a recording: WASAPI loopback of the default output device
//!        (system audio) and WASAPI capture of a microphone, each on its own thread,
//!        delivering stereo f32 frames stamped on the QPC clock. Also the microphone list.
//! WHY:   WASAPI is the native audio API and gives a QPC time for every packet, the clock
//!        Windows.Graphics.Capture stamps frames with, so audio and video line up. Shared
//!        mode asks Windows to convert to 48 kHz stereo float (`AUTOCONVERTPCM`); if a
//!        driver refuses, the device's own mix format is read and converted here
//!        (`audio::to_stereo` + the resampler). A quiet loopback sends no packets, so the
//!        thread writes silence for the gaps (`audio::silence_gap`).
//! WHERE: `win::session`; `win::devices` (mic list).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use rec_core::state::InputDevice;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::core::PCWSTR;

use super::util::{com_init, hns_to_secs, msg, qpc_secs, wide};
use crate::audio::{RATE, SampleFormat, silence_gap, to_stereo};

/// What to capture.
#[derive(Clone, Debug)]
pub enum Endpoint {
    /// Everything the default output device plays.
    Loopback,
    /// A microphone by endpoint id, or the default one.
    Mic(Option<String>),
}

/// Stereo frames from a source: (start on the QPC clock in seconds, sample rate, frames).
pub type Deliver = Box<dyn FnMut(f64, f64, Vec<[f32; 2]>) + Send>;

/// A running capture thread.
pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The shared-mode buffer WASAPI keeps for us (200 ms).
const BUFFER_HNS: i64 = 2_000_000;
/// How often the thread looks for packets.
const POLL: Duration = Duration::from_millis(10);
/// A loopback hole longer than this is filled with silence (seconds).
const SILENCE_AFTER: f64 = 0.1;

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    // SAFETY: standard COM activation of the MMDevice enumerator.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

/// The friendly name of an endpoint.
fn device_name(d: &IMMDevice) -> Option<String> {
    // SAFETY: property-store reads on a live device; the PROPVARIANT is cleared and the
    // allocated string freed.
    unsafe {
        let store = d.OpenPropertyStore(STGM_READ).ok()?;
        let mut v = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let s = PropVariantToStringAlloc(&v).ok();
        let _ = PropVariantClear(&mut v);
        let s = s?;
        let out = s.to_string().ok();
        CoTaskMemFree(Some(s.0 as *const _));
        out
    }
}

fn device_id(d: &IMMDevice) -> Option<String> {
    // SAFETY: `GetId` returns a CoTaskMem string we free after copying.
    unsafe {
        let id = d.GetId().ok()?;
        let out = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const _));
        out
    }
}

/// The active microphones (capture endpoints).
pub fn list_mics() -> Vec<InputDevice> {
    com_init();
    let Ok(e) = enumerator() else {
        return vec![];
    };
    // SAFETY: enumeration on a live enumerator.
    let Ok(all) = (unsafe { e.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }) else {
        return vec![];
    };
    // SAFETY: as above.
    let n = unsafe { all.GetCount() }.unwrap_or(0);
    (0..n)
        .filter_map(|i| {
            // SAFETY: `i` is below the collection's count.
            let d = unsafe { all.Item(i) }.ok()?;
            Some(InputDevice {
                id: device_id(&d)?,
                name: device_name(&d).unwrap_or_else(|| "Microphone".into()),
                connected: true,
            })
        })
        .collect()
}

/// How the device delivers samples.
#[derive(Clone, Copy, Debug)]
struct Format {
    sample: SampleFormat,
    channels: usize,
    rate: f64,
    block: usize,
}

/// 48 kHz stereo float, the format we ask Windows to convert to.
fn wanted_format() -> WAVEFORMATEXTENSIBLE {
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 48_000 * 8,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: (std::mem::size_of::<WAVEFORMATEXTENSIBLE>()
                - std::mem::size_of::<WAVEFORMATEX>()) as u16,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 {
            wValidBitsPerSample: 32,
        },
        dwChannelMask: 0x3,
        SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
    }
}

/// Read a device mix format.
///
/// # Safety
/// `p` points at a valid `WAVEFORMATEX` (and, for `WAVE_FORMAT_EXTENSIBLE`, a full
/// `WAVEFORMATEXTENSIBLE`), as `IAudioClient::GetMixFormat` returns.
unsafe fn read_format(p: *const WAVEFORMATEX) -> Option<Format> {
    // SAFETY: per the contract; packed structs are read unaligned by value.
    let base = unsafe { std::ptr::read_unaligned(p) };
    let tag = u32::from(base.wFormatTag);
    let bits = base.wBitsPerSample;
    let (float, valid) = if tag == WAVE_FORMAT_EXTENSIBLE {
        // SAFETY: per the contract.
        let ext = unsafe { std::ptr::read_unaligned(p.cast::<WAVEFORMATEXTENSIBLE>()) };
        let sub = ext.SubFormat;
        // SAFETY: a union of u16s; every variant is a plain u16.
        let valid = unsafe { ext.Samples.wValidBitsPerSample };
        if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            (true, valid)
        } else if sub == KSDATAFORMAT_SUBTYPE_PCM {
            (false, valid)
        } else {
            return None;
        }
    } else {
        (tag == WAVE_FORMAT_IEEE_FLOAT, bits)
    };
    Some(Format {
        sample: SampleFormat::from_wave(float, bits, valid)?,
        channels: usize::from(base.nChannels.max(1)),
        rate: f64::from(base.nSamplesPerSec),
        block: usize::from(base.nBlockAlign.max(1)),
    })
}

/// Open and start a client on `ep`.
fn open(ep: &Endpoint) -> Result<(IAudioClient, IAudioCaptureClient, Format), String> {
    let e = enumerator().map_err(|e| msg("No audio devices", &e))?;
    // SAFETY: device lookups on a live enumerator; `id` is NUL-terminated.
    let device = unsafe {
        match ep {
            Endpoint::Loopback => e.GetDefaultAudioEndpoint(eRender, eConsole),
            Endpoint::Mic(Some(id)) => {
                let w = wide(id);
                e.GetDevice(PCWSTR(w.as_ptr()))
                    .or_else(|_| e.GetDefaultAudioEndpoint(eCapture, eConsole))
            }
            Endpoint::Mic(None) => e.GetDefaultAudioEndpoint(eCapture, eConsole),
        }
    }
    .map_err(|e| msg("No audio device", &e))?;
    let loopback = if matches!(ep, Endpoint::Loopback) {
        AUDCLNT_STREAMFLAGS_LOOPBACK
    } else {
        0
    };
    // SAFETY: activation of IAudioClient on a live endpoint.
    let client: IAudioClient =
        unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| msg("Audio device", &e))?;
    let want = wanted_format();
    // SAFETY: `want` is a complete WAVEFORMATEXTENSIBLE that outlives the call.
    let converted = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            loopback | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            BUFFER_HNS,
            0,
            (&want as *const WAVEFORMATEXTENSIBLE).cast(),
            None,
        )
    };
    let (client, format) = match converted {
        Ok(()) => (
            client,
            Format {
                sample: SampleFormat::F32,
                channels: 2,
                rate: RATE,
                block: 8,
            },
        ),
        Err(_) => {
            // The driver would not convert: take its mix format as it is.
            // SAFETY: a fresh client from the same device.
            let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
                .map_err(|e| msg("Audio device", &e))?;
            // SAFETY: GetMixFormat returns a CoTaskMem WAVEFORMATEX(-EXTENSIBLE), read and
            // used for Initialize before it is freed.
            unsafe {
                let mix = client.GetMixFormat().map_err(|e| msg("Audio format", &e))?;
                let f = read_format(mix);
                let r =
                    client.Initialize(AUDCLNT_SHAREMODE_SHARED, loopback, BUFFER_HNS, 0, mix, None);
                CoTaskMemFree(Some(mix as *const _));
                r.map_err(|e| msg("Audio device", &e))?;
                (
                    client,
                    f.ok_or_else(|| {
                        "The audio device uses a sample format Cortex can't read".to_string()
                    })?,
                )
            }
        }
    };
    // SAFETY: the client is initialised.
    let capture: IAudioCaptureClient =
        unsafe { client.GetService() }.map_err(|e| msg("Audio capture", &e))?;
    // SAFETY: as above.
    unsafe { client.Start() }.map_err(|e| msg("Audio capture", &e))?;
    Ok((client, capture, format))
}

impl AudioCapture {
    /// Start capturing `ep`; `deliver` gets every buffer on the capture thread. Errors
    /// opening the device are returned here.
    pub fn start(ep: Endpoint, mut deliver: Deliver) -> Result<AudioCapture, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let s = stop.clone();
        let loopback = matches!(ep, Endpoint::Loopback);
        let thread = std::thread::Builder::new()
            .name(
                if loopback {
                    "wasapi-loopback"
                } else {
                    "wasapi-mic"
                }
                .into(),
            )
            .spawn(move || {
                com_init();
                let (client, capture, f) = match open(&ep) {
                    Ok(x) => {
                        let _ = tx.send(Ok(()));
                        x
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                let mut last_end = qpc_secs();
                while !s.load(Ordering::SeqCst) {
                    std::thread::sleep(POLL);
                    loop {
                        // SAFETY: a started capture client.
                        let n = unsafe { capture.GetNextPacketSize() }.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        let mut data: *mut u8 = std::ptr::null_mut();
                        let (mut frames, mut flags, mut qpc) = (0u32, 0u32, 0u64);
                        // SAFETY: out-pointers are valid; the buffer is released below.
                        let got = unsafe {
                            capture.GetBuffer(
                                &mut data,
                                &mut frames,
                                &mut flags,
                                None,
                                Some(&mut qpc),
                            )
                        };
                        if got.is_err() {
                            break;
                        }
                        let bytes = frames as usize * f.block;
                        let silent = flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                        let out = if silent || data.is_null() {
                            vec![[0.0f32; 2]; frames as usize]
                        } else {
                            // SAFETY: WASAPI hands `frames` frames of `block` bytes each.
                            let raw = unsafe { std::slice::from_raw_parts(data, bytes) };
                            to_stereo(raw, f.sample, f.channels)
                        };
                        // SAFETY: releases exactly what GetBuffer returned.
                        let _ = unsafe { capture.ReleaseBuffer(frames) };
                        let t = hns_to_secs(qpc as i64);
                        last_end = t + out.len() as f64 / f.rate;
                        deliver(t, f.rate, out);
                    }
                    // Loopback sends nothing while nothing plays: fill with silence so the
                    // system track and the mix keep moving.
                    if loopback
                        && let Some((t, n)) = silence_gap(last_end, qpc_secs(), SILENCE_AFTER)
                    {
                        last_end = t + n as f64 / RATE;
                        deliver(t, RATE, vec![[0.0; 2]; n]);
                    }
                }
                // SAFETY: stops a started client.
                let _ = unsafe { client.Stop() };
            })
            .map_err(|e| format!("Could not start audio capture: {e}"))?;
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(AudioCapture {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                Err("The audio device did not start".into())
            }
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
