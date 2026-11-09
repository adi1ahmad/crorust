//! UI Shell, Telemetry & Diagnostics Core

pub mod frame_history;
pub mod scroll;
pub mod settings_panel;

#[cfg(feature = "accessibility")]
pub mod accessibility;

pub use frame_history::FrameHistory;
pub use settings_panel::SettingsPanel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenTier {
    Compact,
    Medium,
    Expanded,
}

impl ScreenTier {
    pub fn from_ui(ui: &egui::Ui) -> Self {
        let width = ui.available_width();
        if width < 500.0 || crate::is_mobile(ui.ctx()) {
            Self::Compact
        } else if width < 800.0 {
            Self::Medium
        } else {
            Self::Expanded
        }
    }

    pub fn is_compact(&self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// Renders a horizontal scrolling strip where child items are guaranteed never to wrap vertically.
///
/// This primitive solves the mobile horizontal-space constraint where inherited `TextWrapMode::Wrap`
/// causes horizontal buttons, pills, and tabs to collapse into vertical single-character columns
/// when approaching the right screen boundary.
///
/// Inside this strip:
/// - `wrap_mode` is locked to `TextWrapMode::Extend`
/// - Child items preserve natural single-line horizontal dimensions
/// - The strip smoothly scrolls horizontally on touchscreens and desktops
pub fn responsive_scroll_strip<R>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 550.0;

    egui::ScrollArea::horizontal()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .scroll_bar_visibility(if is_mobile {
            egui::scroll_area::ScrollBarVisibility::AlwaysHidden
        } else {
            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
        })
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                add_contents(ui)
            })
            .inner
        })
        .inner
}
