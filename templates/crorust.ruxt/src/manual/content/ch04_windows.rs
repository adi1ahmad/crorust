use egui::{RichText, Ui};

pub fn render_chapter(
    ui: &mut Ui,
    target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(RichText::new("🪟 Multi-Window Workspace & Touch Ergonomics").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Manage multiple floating panels, organize tiling, and optimize for mobile viewports.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. Floating Windows in egui").size(17.0));
    ui.label(
        "egui::Window allows creating movable, resizable, collapsable floating surfaces. \
        ruxt integrates these into a coherent multi-window workstation.\n\n\
        On mobile screens, opening multiple windows creates visual clutter. ruxt solves this \
        by keeping only the living manual active on entry, closing the sidebar by default, \
        and smoothly focusing target windows on demand."
    );

    ui.add_space(16.0);

    // Interactive Window Actions
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(RichText::new("Quick Window Commands").strong());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("📐 Auto-Tile Windows").clicked() {
                    ui.ctx().memory_mut(|m| m.reset_areas());
                }
                if ui.button("⏰ Launch Analog Clock").clicked() {
                    *target_to_launch = Some("⏰ Analog Clock");
                }
                if ui.button("ℹ️ Launch About Window").clicked() {
                    *target_to_launch = Some("About crorust.ruxt");
                }
            });
        });
}
