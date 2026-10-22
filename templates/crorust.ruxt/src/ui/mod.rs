//! UI Shell, Telemetry & Diagnostics Core

pub mod frame_history;
pub mod settings_panel;

#[cfg(feature = "accessibility")]
pub mod accessibility;

pub use frame_history::FrameHistory;
pub use settings_panel::SettingsPanel;
