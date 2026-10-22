use egui::{Color32, RichText, Ui};

pub fn render_chapter(
    ui: &mut Ui,
    target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(RichText::new("🚀 Application Architecture, Persistence & Packaging").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Cross-session serialization, hermetic builds, and multi-platform packaging with crorust.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. Cross-Session State Persistence").size(17.0));
    ui.label(
        "By enabling the 'persistence' feature with serde, ruxt automatically serializes window layouts, \
        scroll states, and application models to disk across restarts (or to browser localStorage in WASM):"
    );

    crate::rust_view_ui(
        ui,
        r#"#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct AppState {
    pub username: String,
    pub volume: f32,
}

impl eframe::App for RuxtApp {
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }
}"#,
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("2. Zero External IDE Builds with crorust").size(17.0));
    ui.label(
        "crorust packages your application for Android (.apk, .aab), iOS (.app, .ipa), \
        macOS (.dmg), Windows (.msix), and Linux (.AppImage) completely from the command line \
        with pure-Rust packaging crates and hermetic Clang/LLVM toolchains."
    );

    ui.add_space(16.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("🎨 Verify Monitor Color Calibration").strong().size(15.0));
                    ui.label(
                        RichText::new("Inspect gamma, sRGB gradients, and high-DPI scaling.")
                            .size(12.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("🎨 Open Calibration ➔").strong()).clicked() {
                        *target_to_launch = Some("🎨 Display Calibration");
                    }
                });
            });
        });
}
