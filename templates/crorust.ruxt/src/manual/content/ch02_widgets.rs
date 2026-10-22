use egui::{Color32, RichText, Ui};
use crate::manual::widgets::ManualPlaygroundState;

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(RichText::new("🎛️ Essential Widgets & Interactive Controls").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Explore standard egui controls with live feedback.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(
        "Every widget in egui implements the egui::Widget trait. Below is a live interactive sampler \
        containing core widgets. Try them right here inside the manual:"
    );

    ui.add_space(12.0);

    // Live Sampler Card
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.checkbox(&mut state.toggle_enabled, "Enable Feature Toggle");

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Scalar Value:");
                ui.add(egui::Slider::new(&mut state.slider_value, 0.0..=100.0).suffix(" units"));
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Accent Color:");
                ui.color_edit_button_srgba(&mut state.color);
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Radio Select:");
                ui.radio_value(&mut state.selected_radio, 0, "Alpha");
                ui.radio_value(&mut state.selected_radio, 1, "Beta");
                ui.radio_value(&mut state.selected_radio, 2, "Gamma");
            });
        });

    ui.add_space(18.0);

    // Progressive Guided Step Launcher
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("🚀 Ready to Explore the Full Widget Gallery?").strong().size(15.0));
                    ui.label(
                        RichText::new("Open the complete Widget Showcase containing 25+ real-world controls.")
                            .size(12.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Open '📦 Widget Showcase' ➔").strong()).clicked() {
                        *target_to_launch = Some("📦 Widget Showcase");
                    }
                });
            });
        });
}
