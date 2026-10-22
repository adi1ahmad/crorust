use egui::{Color32, RichText, Ui};
use crate::manual::widgets::ManualPlaygroundState;

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(RichText::new("📐 Layout Containers, Grids & Responsiveness").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Learn how egui packs controls horizontally, vertically, and into responsive strips.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. Core Layout Builders").size(17.0));
    ui.label(
        "egui gives you expressive immediate-mode layout primitives:\n\
        • ui.horizontal(|ui| ...): Flows items left-to-right.\n\
        • ui.vertical(|ui| ...): Stacks items top-to-bottom.\n\
        • egui::ScrollArea: Adds virtualized horizontal or vertical scrolling.\n\
        • egui::Grid: Creates aligned 2D tables with automatic cell sizing."
    );

    ui.add_space(16.0);

    // Interactive Layout Resizer Card
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.label(RichText::new("🧪 Interactive Playground: Proportional Splitter").strong().color(Color32::from_rgb(31, 111, 235)));
            ui.add_space(6.0);
            ui.label("Drag the slider to dynamically divide available space between two panels in real time:");

            ui.add_space(8.0);
            ui.add(egui::Slider::new(&mut state.split_ratio, 0.1..=0.9).text("Split Ratio").show_value(true));

            ui.add_space(8.0);
            let total_w = ui.available_width();
            let left_w = (total_w * state.split_ratio - 4.0).max(40.0);
            let right_w = (total_w * (1.0 - state.split_ratio) - 4.0).max(40.0);

            ui.horizontal(|ui| {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(30, 40, 55))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(left_w);
                        ui.label(RichText::new(format!("Left Panel ({:.0}px)", left_w)).small().color(Color32::WHITE));
                    });
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(55, 40, 30))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(right_w);
                        ui.label(RichText::new(format!("Right Panel ({:.0}px)", right_w)).small().color(Color32::WHITE));
                    });
            });
        });

    ui.add_space(18.0);

    // Progressive Step Launcher
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("🚀 Ready to Explore Advanced Strip Layouts?").strong().size(15.0));
                    ui.label(
                        RichText::new("Open the Strip Layout Demo with resizable splitters and cell headers.")
                            .size(12.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Open '📏 Strip Layout Demo' ➔").strong()).clicked() {
                        *target_to_launch = Some("📏 Strip Layout Demo");
                    }
                });
            });
        });
}
