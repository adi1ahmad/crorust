use egui::{Color32, RichText, Ui};
use crate::manual::widgets::ManualPlaygroundState;

pub fn render_chapter(ui: &mut Ui, state: &mut ManualPlaygroundState) {
    ui.heading(RichText::new("🏛️ The Immediate-Mode Mental Model").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Welcome to ruxt! This interactive manual guides you step-by-step through the framework.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. Retained-Mode vs. Immediate-Mode").size(17.0));
    ui.label(
        "In traditional retained-mode frameworks (HTML DOM, Flutter, Qt, SwiftUI), you create long-lived widget \
        objects and attach callbacks. State lives in two places: your code and the UI tree.\n\n\
        In ruxt's immediate-mode architecture, there is NO persistent widget tree. Your UI code executes from \
        top to bottom on every frame. When a button is clicked, it returns true in that exact frame:"
    );

    crate::rust_view_ui(
        ui,
        r#"if ui.button("Click Me").clicked() {
    state.click_count += 1;
}"#,
    );

    ui.add_space(16.0);

    // Interactive In-Manual Live Playground
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Interactive Playground: Live Reactive State")
                    .strong()
                    .color(Color32::from_rgb(31, 111, 235)),
            );
            ui.add_space(6.0);
            ui.label("Mutate state below and observe immediate-mode reactivity with zero boilerplate:");

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Your Name:");
                ui.text_edit_singleline(&mut state.user_name);
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(format!("🦀 Increment Count ({})", state.click_count)).clicked() {
                    state.click_count += 1;
                }
                if ui.button("Reset").clicked() {
                    state.click_count = 0;
                }
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "Live Result: Hello, {}! You clicked the button {} time(s).",
                    state.user_name, state.click_count
                ))
                .strong()
                .color(ui.visuals().strong_text_color()),
            );
        });

    ui.add_space(16.0);
    ui.heading(RichText::new("2. The Three Principles of ruxt").size(17.0));
    ui.label("• 100% Pure Rust: No C/C++ build dependencies, zero external GUI runtime.\n\
              • Hermetic Cross-Compilation: Packaged via crorust without installing 15GB external IDEs.\n\
              • Hardware Accelerated: Tessellated into GPU vertex/index buffers via wgpu or glow.");
}
