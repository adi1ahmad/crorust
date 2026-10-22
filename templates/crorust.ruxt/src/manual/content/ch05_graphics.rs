use egui::{Color32, Pos2, RichText, Stroke, Ui, Vec2};
use crate::manual::widgets::ManualPlaygroundState;

pub fn render_chapter(
    ui: &mut Ui,
    state: &mut ManualPlaygroundState,
    target_to_launch: &mut Option<&'static str>,
) {
    ui.heading(RichText::new("🎨 2D Vector Painting & 3D Graphics Pipelines").size(24.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("Render custom 2D primitives and hardware-accelerated 3D viewports.")
            .size(13.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. The Immediate Painter").size(17.0));
    ui.label(
        "Using ui.painter(), you can draw lines, circles, text, and Bezier curves directly onto the screen. \
        Try sketching below inside this mini interactive drawing canvas:"
    );

    ui.add_space(12.0);

    // Interactive Drawing Surface
    egui::Frame::canvas(ui.style())
        .fill(Color32::from_rgb(18, 22, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(45, 55, 72)))
        .inner_margin(8.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 160.0), egui::Sense::drag());
            let rect = response.rect;

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if rect.contains(pointer_pos) {
                    state.canvas_points.push(Some(pointer_pos));
                }
            } else if !state.canvas_points.is_empty() && state.canvas_points.last().unwrap().is_some() {
                state.canvas_points.push(None);
            }

            // Draw line segments
            let mut last_pt: Option<Pos2> = None;
            for pt_opt in &state.canvas_points {
                if let Some(pt) = pt_opt {
                    if let Some(prev) = last_pt {
                        painter.line_segment([prev, *pt], Stroke::new(2.5, Color32::from_rgb(31, 111, 235)));
                    }
                    last_pt = Some(*pt);
                } else {
                    last_pt = None;
                }
            }

            if state.canvas_points.is_empty() {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✍️ Draw here with mouse or touch",
                    egui::FontId::proportional(14.0),
                    Color32::from_rgb(120, 130, 145),
                );
            }
        });

    ui.add_space(6.0);
    if ui.button("Clear Canvas").clicked() {
        state.canvas_points.clear();
    }

    ui.add_space(16.0);

    // Progressive Launchers
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.5, Color32::from_rgb(31, 111, 235)))
        .inner_margin(14.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("🚀 Explore Dedicated Graphics Demos").strong().size(15.0));
                    ui.label(
                        RichText::new("Open parametric Bezier curves or hardware-accelerated 3D shaders.")
                            .size(12.5)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📐 Bezier Curves ➔").strong()).clicked() {
                        *target_to_launch = Some("📐 Bezier Curves");
                    }
                    if ui.button(RichText::new("🧊 3D Viewport ➔").strong()).clicked() {
                        *target_to_launch = Some("🧊 3D Viewport");
                    }
                });
            });
        });
}
