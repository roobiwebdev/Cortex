//! SOURCE OF TRUTH KEYWORDS: Encoder, IMFSinkWriter, H.264, HEVC, AAC, MP4, hardware transforms, DXGI device manager, hevc_encoder_available, fragmented MP4
//! WHAT:  The file writer: a Media Foundation sink writer with one H.264 or HEVC video
//!        stream fed NV12 D3D11 textures, and up to three AAC audio streams fed 48 kHz
//!        stereo 16-bit PCM, written as MPEG-4.
//! WHY:   The sink writer picks the GPU's encoder MFT (NVENC, Quick Sync, AMF) when
//!        `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS` is set and it shares our D3D11 device
//!        through the DXGI device manager, so frames are never copied to system memory.
//!        Microsoft's software H.264 MFT is the fallback. Crash safety: a plain MP4 keeps
//!        its index (`moov`) at the end, so a crash before `finish` leaves an unplayable
//!        file. `fragmented` switches to Media Foundation's fragmented MP4 sink instead
//!        (`MFTranscodeContainerType_FMPEG4`), unverified, so it is opt-in for now.
//! WHERE: `win::session`; `hevc_encoder_available` for `PlatformCapabilities::hevc`.

use std::path::Path;

use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{Interface, PCWSTR, Result};

use super::d3d::Gpu;
use super::util::wide;
use crate::plan::{EncoderConfig, VideoCodec};

/// AAC at 192 kb/s (24000 bytes a second, one of the four rates Microsoft's encoder takes).
const AAC_BYTES_PER_SEC: u32 = 24_000;

fn pack(hi: u32, lo: u32) -> u64 {
    (u64::from(hi) << 32) | u64::from(lo)
}

fn video_type(cfg: &EncoderConfig, subtype: &windows::core::GUID) -> Result<IMFMediaType> {
    // SAFETY: plain attribute setters on a fresh media type.
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(cfg.width, cfg.height))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(cfg.fps, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        Ok(t)
    }
}

fn audio_type(subtype: &windows::core::GUID, bytes_per_sec: u32) -> Result<IMFMediaType> {
    // SAFETY: plain attribute setters on a fresh media type.
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_sec)?;
        if *subtype == MFAudioFormat_PCM {
            t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
        }
        Ok(t)
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: Encoder, sink writer, write_video, write_audio, finish
 * WHAT:  One MP4 being written.
 * WHY:   All Media Foundation writing in one type; callers hand it textures and PCM.
 * WHERE: `win::session` (behind a mutex shared by the video and audio threads).
 */
pub struct Encoder {
    writer: IMFSinkWriter,
    video: u32,
    audio: Vec<u32>,
}

// SAFETY: the sink writer is free-threaded; the session serialises calls with a mutex.
unsafe impl Send for Encoder {}

impl Encoder {
    /// Open `path` for writing. `gpu`: share this device with the encoder (hardware path);
    /// None writes through system memory. `audio_tracks`: number of AAC streams.
    pub fn new(
        path: &Path,
        cfg: &EncoderConfig,
        audio_tracks: usize,
        gpu: Option<&Gpu>,
        fragmented: bool,
    ) -> Result<Encoder> {
        let url = wide(&path.display().to_string());
        // SAFETY: attribute and factory calls with valid pointers; `url` is NUL-terminated.
        unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 4)?;
            let attrs = attrs.ok_or_else(windows::core::Error::empty)?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            // The video and audio threads share one writer behind a mutex. With throttling
            // on, WriteSample can block one stream until the other catches up (the muxer
            // interleaves), while the mutex keeps the other stream out: a deadlock. The
            // video thread bounds its own backlog instead (`video_backlog`).
            attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
            attrs.SetGUID(
                &MF_TRANSCODE_CONTAINERTYPE,
                if fragmented {
                    &MFTranscodeContainerType_FMPEG4
                } else {
                    &MFTranscodeContainerType_MPEG4
                },
            )?;
            if let Some(gpu) = gpu {
                let mut token = 0u32;
                let mut manager = None;
                MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
                let manager: IMFDXGIDeviceManager =
                    manager.ok_or_else(windows::core::Error::empty)?;
                manager.ResetDevice(&gpu.device, token)?;
                attrs.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &manager)?;
            }
            let writer = MFCreateSinkWriterFromURL(PCWSTR(url.as_ptr()), None, &attrs)?;

            let subtype = match cfg.codec {
                VideoCodec::H264 => MFVideoFormat_H264,
                VideoCodec::Hevc => MFVideoFormat_HEVC,
            };
            let out = video_type(cfg, &subtype)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, cfg.bitrate)?;
            if cfg.codec == VideoCodec::H264 {
                out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            }
            let video = writer.AddStream(&out)?;
            let input = video_type(cfg, &MFVideoFormat_NV12)?;
            input.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
            input.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
            writer.SetInputMediaType(video, &input, None)?;

            let mut audio = vec![];
            for _ in 0..audio_tracks {
                let out = audio_type(&MFAudioFormat_AAC, AAC_BYTES_PER_SEC)?;
                let i = writer.AddStream(&out)?;
                let input = audio_type(&MFAudioFormat_PCM, 48_000 * 4)?;
                writer.SetInputMediaType(i, &input, None)?;
                audio.push(i);
            }
            writer.BeginWriting()?;
            Ok(Encoder {
                writer,
                video,
                audio,
            })
        }
    }

    /// Write one frame: an NV12 texture shown from `time` for `duration` (100 ns units).
    pub fn write_video(&self, tex: &ID3D11Texture2D, time: i64, duration: i64) -> Result<()> {
        // SAFETY: the texture is a live NV12 texture on the device the writer shares;
        // the sample keeps it alive until the encoder is done with it.
        unsafe {
            let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, tex, 0, false)?;
            if let Ok(b2) = buffer.cast::<IMF2DBuffer>() {
                buffer.SetCurrentLength(b2.GetContiguousLength()?)?;
            }
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            self.writer.WriteSample(self.video, &sample)
        }
    }

    /// Video frames handed to the writer and not yet consumed by the encoder.
    pub fn video_backlog(&self) -> u64 {
        let mut st = MF_SINK_WRITER_STATISTICS {
            cb: std::mem::size_of::<MF_SINK_WRITER_STATISTICS>() as u32,
            ..Default::default()
        };
        // SAFETY: `st` is a valid out-structure with its size set.
        match unsafe { self.writer.GetStatistics(self.video, &mut st) } {
            Ok(()) => st
                .qwNumSamplesReceived
                .saturating_sub(st.qwNumSamplesEncoded),
            Err(_) => 0,
        }
    }

    /// Write interleaved 16-bit stereo PCM to audio track `track`, starting at `time`
    /// (100 ns units).
    pub fn write_audio(&self, track: usize, pcm: &[u8], time: i64, duration: i64) -> Result<()> {
        let Some(&stream) = self.audio.get(track) else {
            return Ok(());
        };
        if pcm.is_empty() {
            return Ok(());
        }
        let len = u32::try_from(pcm.len()).map_err(|_| windows::core::Error::empty())?;
        // SAFETY: the buffer is locked for exactly `len` bytes while we copy into it.
        unsafe {
            let buffer = MFCreateMemoryBuffer(len)?;
            let mut p: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut p, None, None)?;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(pcm.as_ptr(), p, pcm.len());
            }
            buffer.Unlock()?;
            buffer.SetCurrentLength(len)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            self.writer.WriteSample(stream, &sample)
        }
    }

    /// Write the index and close the file.
    pub fn finish(self) -> Result<()> {
        // SAFETY: plain call on a live writer.
        unsafe { self.writer.Finalize() }
    }
}

/// An HEVC encoder MFT is registered (usually the GPU's).
pub fn hevc_encoder_available() -> bool {
    let out = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_HEVC,
    };
    let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    let flags = MFT_ENUM_FLAG(
        MFT_ENUM_FLAG_SYNCMFT.0
            | MFT_ENUM_FLAG_ASYNCMFT.0
            | MFT_ENUM_FLAG_HARDWARE.0
            | MFT_ENUM_FLAG_SORTANDFILTER.0,
    );
    // SAFETY: out-pointers are valid; the array and its entries are released below.
    let ok = unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            flags,
            None,
            Some(&out),
            &mut list,
            &mut count,
        )
    }
    .is_ok();
    if !list.is_null() {
        for i in 0..count as usize {
            // SAFETY: `list` holds `count` activation objects; taking each releases it.
            drop(unsafe { (*list.add(i)).take() });
        }
        // SAFETY: the array was allocated by MFTEnumEx with CoTaskMemAlloc.
        unsafe { CoTaskMemFree(Some(list as *const _)) };
    }
    ok && count > 0
}
