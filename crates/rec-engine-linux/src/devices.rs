//! SOURCE OF TRUTH KEYWORDS: DeviceList, GStreamer DeviceMonitor, microphones, cameras, PulseAudio, PipeWire, V4L2, device ids
//! WHAT:  Lists microphones and cameras through GStreamer's `DeviceMonitor`, which talks to
//!        PulseAudio (or pipewire-pulse), PipeWire and V4L2, and turns each into an
//!        `InputDevice` whose id `pipeline::source_for_id` can open.
//! WHY:   One API covers every audio server and camera stack a distro may use. PulseAudio
//!        and V4L2 entries win over PipeWire's duplicates of the same hardware, because their
//!        names and paths are stable across reboots. Sink monitors are left out of the mic
//!        list: system audio is its own source.
//! WHERE: `engine` (devices, the device watcher).

use gstreamer as gst;
use gstreamer::prelude::*;
use rec_core::state::InputDevice;

use crate::pipeline::{PIPEWIRE_ID, PULSE_ID, V4L2_ID};

pub struct DeviceList {
    monitor: gst::DeviceMonitor,
}

fn prop_str(d: &gst::Device, name: &str) -> Option<String> {
    d.find_property(name)?;
    let v = d.property_value(name);
    v.get::<Option<String>>().ok().flatten()
}

fn props_str(d: &gst::Device, key: &str) -> Option<String> {
    d.properties()?.get::<String>(key).ok()
}

/// The id for a device, or None to skip it (unknown provider, a sink monitor).
fn id_for(d: &gst::Device) -> Option<(String, &'static str)> {
    let kind = d.type_().name();
    match kind {
        "GstPulseDevice" => {
            if props_str(d, "device.class").as_deref() == Some("monitor") {
                return None;
            }
            Some((
                format!("{PULSE_ID}{}", prop_str(d, "internal-name")?),
                "pulse",
            ))
        }
        "GstV4l2Device" => Some((format!("{V4L2_ID}{}", prop_str(d, "device-path")?), "v4l2")),
        "GstPipeWireDevice" => {
            if props_str(d, "media.class").is_some_and(|c| c.contains("Sink")) {
                return None;
            }
            Some((
                format!("{PIPEWIRE_ID}{}", props_str(d, "node.name")?),
                "pipewire",
            ))
        }
        _ => None,
    }
}

impl DeviceList {
    /// Starts watching; None when GStreamer has no device providers.
    pub fn start() -> Option<DeviceList> {
        let monitor = gst::DeviceMonitor::new();
        monitor.add_filter(Some("Audio/Source"), None);
        monitor.add_filter(Some("Video/Source"), None);
        monitor.start().ok()?;
        Some(DeviceList { monitor })
    }

    /// (cameras, mics) right now.
    pub fn current(&self) -> (Vec<InputDevice>, Vec<InputDevice>) {
        let mut cams: Vec<(InputDevice, &str)> = vec![];
        let mut mics: Vec<(InputDevice, &str)> = vec![];
        for d in self.monitor.devices() {
            let Some((id, api)) = id_for(&d) else {
                continue;
            };
            let dev = InputDevice {
                id,
                name: d.display_name().to_string(),
                connected: true,
            };
            let class = d.device_class();
            let list = if class.contains("Video/Source") {
                &mut cams
            } else if class.contains("Audio/Source") {
                &mut mics
            } else {
                continue;
            };
            if !list.iter().any(|(x, _)| x.id == dev.id) {
                list.push((dev, api));
            }
        }
        // Prefer the stable APIs when both list the same hardware.
        let pick = |list: Vec<(InputDevice, &str)>, preferred: &str| {
            let any_preferred = list.iter().any(|(_, a)| *a == preferred);
            list.into_iter()
                .filter(|(_, a)| !any_preferred || *a == preferred)
                .map(|(d, _)| d)
                .collect::<Vec<_>>()
        };
        (pick(cams, "v4l2"), pick(mics, "pulse"))
    }
}

impl Drop for DeviceList {
    fn drop(&mut self) {
        self.monitor.stop();
    }
}
