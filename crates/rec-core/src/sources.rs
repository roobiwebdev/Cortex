//! SOURCE OF TRUTH KEYWORDS: plan_sources, SourcePlan, graceful degradation, missing permission, camera, microphone, screen, TCC
//! WHAT:  Decides which capture sources a session may open given the settings and the
//!        current privacy (TCC) grants.
//! WHY:   A missing camera or microphone grant must not stop the preview or a recording:
//!        the session runs with what is permitted and the UI offers an Allow button for the
//!        rest. Only a source the scene cannot do without (the screen for screen scenes,
//!        the camera for the camera-only scene) blocks it. Pure, so it is unit tested
//!        without macOS.
//! WHERE: `rec-engine-macos::engine` (preview and recording options).

use crate::settings::Scene;
use crate::state::{Permission, PermissionState, Permissions};

/// The sources a session opens. `dropped` lists the wanted ones left out for lack of a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePlan {
    /// Capture the screen (and system audio). False only for the camera-only scene
    /// without a Screen Recording grant.
    pub screen: bool,
    pub camera: bool,
    pub mic: bool,
    pub dropped: Vec<Permission>,
}

/// Which sources `scene` with the mic `mic_enabled` may open under `perms`, or the grants
/// that block it entirely. `screen_free` is true for the test pattern, which needs no
/// Screen Recording grant.
pub fn plan_sources(
    scene: Scene,
    mic_enabled: bool,
    perms: &Permissions,
    screen_free: bool,
) -> Result<SourcePlan, Vec<Permission>> {
    let ok = |p: Permission| perms.get(p) == PermissionState::Granted;
    let screen_ok = screen_free || ok(Permission::Screen);
    let camera_ok = ok(Permission::Camera);
    let mut dropped = vec![];
    let (screen, camera) = match scene {
        Scene::Screen | Scene::ScreenCam if !screen_ok => return Err(vec![Permission::Screen]),
        Scene::Screen => (true, false),
        Scene::ScreenCam => {
            if !camera_ok {
                dropped.push(Permission::Camera);
            }
            (true, camera_ok)
        }
        // Camera only: never fall back to showing the screen the user chose not to show.
        Scene::Cam if !camera_ok => return Err(vec![Permission::Camera]),
        // The screen still runs when allowed, for system audio and live scene switches.
        Scene::Cam => (screen_ok, true),
    };
    let mic = mic_enabled && ok(Permission::Microphone);
    if mic_enabled && !mic {
        dropped.push(Permission::Microphone);
    }
    Ok(SourcePlan {
        screen,
        camera,
        mic,
        dropped,
    })
}

/// A short, user-facing note for sources a recording runs without.
pub fn dropped_note(dropped: &[Permission]) -> Option<String> {
    let names: Vec<&str> = dropped
        .iter()
        .filter_map(|p| match p {
            Permission::Microphone => Some("microphone"),
            Permission::Camera => Some("camera"),
            Permission::Screen => None,
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(format!(
        "Recording without the {}: Cortex doesn't have permission. Allow it in Settings › Privacy.",
        names.join(" or ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use PermissionState::{Denied, Granted, NotDetermined};

    fn perms(
        screen: PermissionState,
        microphone: PermissionState,
        camera: PermissionState,
    ) -> Permissions {
        Permissions {
            screen,
            microphone,
            camera,
        }
    }

    #[test]
    fn everything_granted_opens_everything() {
        let p = plan_sources(
            Scene::ScreenCam,
            true,
            &perms(Granted, Granted, Granted),
            false,
        );
        assert_eq!(
            p,
            Ok(SourcePlan {
                screen: true,
                camera: true,
                mic: true,
                dropped: vec![]
            })
        );
    }

    #[test]
    fn missing_camera_and_mic_degrade_screen_cam() {
        let p = plan_sources(
            Scene::ScreenCam,
            true,
            &perms(Granted, NotDetermined, Denied),
            false,
        );
        assert_eq!(
            p,
            Ok(SourcePlan {
                screen: true,
                camera: false,
                mic: false,
                dropped: vec![Permission::Camera, Permission::Microphone]
            })
        );
    }

    #[test]
    fn mic_off_is_not_dropped() {
        let p = plan_sources(Scene::Screen, false, &perms(Granted, Denied, Denied), false).unwrap();
        assert!(!p.mic && !p.camera && p.dropped.is_empty());
    }

    #[test]
    fn screen_scenes_need_screen() {
        for scene in [Scene::Screen, Scene::ScreenCam] {
            assert_eq!(
                plan_sources(scene, true, &perms(NotDetermined, Granted, Granted), false),
                Err(vec![Permission::Screen])
            );
        }
    }

    #[test]
    fn test_pattern_needs_no_screen_grant() {
        let p = plan_sources(Scene::Screen, false, &perms(Denied, Denied, Denied), true).unwrap();
        assert!(p.screen);
    }

    #[test]
    fn camera_only_runs_without_screen_grant() {
        let p = plan_sources(Scene::Cam, true, &perms(Denied, Granted, Granted), false).unwrap();
        assert_eq!(
            p,
            SourcePlan {
                screen: false,
                camera: true,
                mic: true,
                dropped: vec![]
            }
        );
        let p = plan_sources(Scene::Cam, false, &perms(Granted, Denied, Granted), false).unwrap();
        assert!(p.screen && p.camera);
    }

    #[test]
    fn camera_only_needs_camera() {
        assert_eq!(
            plan_sources(Scene::Cam, true, &perms(Granted, Granted, Denied), false),
            Err(vec![Permission::Camera])
        );
    }

    #[test]
    fn note_names_dropped_sources() {
        assert_eq!(dropped_note(&[]), None);
        assert!(
            dropped_note(&[Permission::Microphone])
                .unwrap()
                .contains("without the microphone:")
        );
        assert!(
            dropped_note(&[Permission::Camera, Permission::Microphone])
                .unwrap()
                .contains("camera or microphone")
        );
    }
}
