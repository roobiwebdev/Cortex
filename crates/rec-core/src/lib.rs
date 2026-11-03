//! Platform-neutral core of the Cortex desktop recorder: the settings schema,
//! the engine state and API, and settings persistence. No Apple code, so a
//! Windows engine can implement the same API later. With the `specta` feature
//! every public type derives `specta::Type` for tauri-specta.

pub mod api;
pub mod library;
pub mod output;
pub mod overlay;
pub mod platform;
pub mod settings;
pub mod sources;
pub mod state;
pub mod store;

pub use api::{Action, EngineError, EngineEvent, EventSink, RecorderEngine};
pub use platform::PlatformCapabilities;
pub use settings::RecorderSettings;
pub use state::{EngineState, EngineStatus};

#[cfg(all(test, feature = "specta"))]
mod specta_tests {
    fn is_type<T: specta::Type>() {}

    #[test]
    fn ipc_types_derive_specta() {
        is_type::<crate::RecorderSettings>();
        is_type::<crate::EngineState>();
        is_type::<crate::EngineEvent>();
        is_type::<crate::EngineError>();
        is_type::<crate::PlatformCapabilities>();
    }
}
