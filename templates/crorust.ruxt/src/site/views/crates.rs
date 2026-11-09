use super::crate_data::{ALL_CRATES, CrateId, CrateInfo};
use crate::navigation::NavigationState;
use egui::{Color32, RichText, Ui};

pub struct CratesExplorerView {
    selected_crate_id: CrateId,
}

impl Default for CratesExplorerView {
    fn default() -> Self {
        Self {
            selected_crate_id: CrateId::CrorustCli,
        }
    }
}

impl CratesExplorerView {
    pub fn ui(&mut self, ui: &mut Ui, _nav: &mut NavigationState) {
        ui.heading(
            RichText::new("In-Tree Crates Architecture")
                .size(26.0)
                .strong(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Explore the 8 pure-Rust crates powering crorust cross-compilation and packaging.",
            )
            .size(14.0)
            .weak(),
        );
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 550.0;

        crate::ui::responsive_scroll_strip(ui, "crate_pills_scroll", |ui| {
            for c in ALL_CRATES {
                let selected = self.selected_crate_id == c.id;
                let text_color = if selected {
                    Color32::WHITE
                } else {
                    ui.visuals().text_color()
                };

                let fill = if selected {
                    Color32::from_rgb(31, 111, 235)
                } else {
                    ui.visuals().faint_bg_color
                };

                let stroke = if selected {
                    egui::Stroke::new(1.0, Color32::from_rgb(88, 166, 255))
                } else {
                    ui.visuals().window_stroke()
                };

                let btn = egui::Button::new(
                    RichText::new(c.name)
                        .strong()
                        .size(if is_mobile { 12.5 } else { 13.0 })
                        .color(text_color),
                )
                .fill(fill)
                .stroke(stroke)
                .corner_radius(6.0)
                .min_size(egui::vec2(0.0, if is_mobile { 36.0 } else { 30.0 }));

                if ui.add(btn).clicked() {
                    self.selected_crate_id = c.id;
                }
            }
        });

        ui.add_space(12.0);

        let active_crate = ALL_CRATES
            .iter()
            .find(|c| c.id == self.selected_crate_id)
            .unwrap_or(&ALL_CRATES[0]);

        render_crate_detail(ui, active_crate);
    }
}

fn render_crate_detail(ui: &mut Ui, info: &CrateInfo) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            let is_compact = crate::ui::ScreenTier::from_ui(ui).is_compact();
            if is_compact {
                ui.heading(RichText::new(info.name).size(20.0).strong());
                ui.label(RichText::new(info.title).size(14.0).weak());
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("📖 docs.rs").small()).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                            "https://docs.rs/crorust/latest/crorust/",
                        ));
                    }
                    if ui.button(RichText::new("📦 crates.io").small()).clicked() {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab("https://crates.io/crates/crorust"));
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new(info.name).size(22.0).strong());
                    ui.label(RichText::new("—").size(15.0).weak());
                    ui.label(RichText::new(info.title).size(15.0).weak());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("📖 docs.rs").small()).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(
                                "https://docs.rs/crorust/latest/crorust/",
                            ));
                        }
                        if ui.button(RichText::new("📦 crates.io").small()).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(
                                "https://crates.io/crates/crorust",
                            ));
                        }
                    });
                });
            }

            ui.add_space(8.0);
            ui.label(RichText::new(info.summary).size(14.0));
            ui.add_space(12.0);

            ui.heading(RichText::new("Architectural Responsibilities").size(16.0));
            ui.label(
                RichText::new(info.details)
                    .size(13.5)
                    .color(ui.visuals().text_color()),
            );

            ui.add_space(14.0);
            ui.label(RichText::new("Formats Handled:").strong().size(13.5));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                let is_dark = ui.visuals().dark_mode;
                let (fmt_text_color, fmt_fill, fmt_stroke) = if is_dark {
                    (
                        Color32::from_rgb(120, 185, 255),
                        Color32::from_rgba_premultiplied(31, 111, 235, 30),
                        egui::Stroke::new(1.0, Color32::from_rgb(45, 90, 150)),
                    )
                } else {
                    (
                        Color32::from_rgb(9, 105, 218),
                        Color32::from_rgba_premultiplied(31, 111, 235, 18),
                        egui::Stroke::new(1.0, Color32::from_rgb(175, 210, 250)),
                    )
                };

                for fmt in info.formats {
                    let btn = egui::Button::new(
                        RichText::new(*fmt)
                            .monospace()
                            .size(12.0)
                            .color(fmt_text_color),
                    )
                    .fill(fmt_fill)
                    .stroke(fmt_stroke)
                    .corner_radius(4.0);
                    ui.add(btn);
                }
            });

            ui.add_space(12.0);
            ui.label(RichText::new("Crate Dependencies:").strong().size(13.5));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for dep in info.dependencies {
                    let btn = egui::Button::new(
                        RichText::new(*dep)
                            .monospace()
                            .size(12.0)
                            .color(ui.visuals().text_color()),
                    )
                    .fill(ui.visuals().faint_bg_color)
                    .stroke(ui.visuals().window_stroke())
                    .corner_radius(4.0);
                    ui.add(btn);
                }
            });

            ui.add_space(16.0);
            ui.heading(RichText::new("Example Usage / CLI Syntax").size(16.0));
            crate::rust_view_ui(ui, info.sample_code);
        });
}
