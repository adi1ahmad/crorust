use egui::{Color32, RichText, ScrollArea, Ui};
use super::crate_data::{ALL_CRATES, CrateId, CrateInfo};
use crate::navigation::NavigationState;

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
        ui.heading(RichText::new("In-Tree Crates Architecture").size(26.0).strong());
        ui.add_space(6.0);
        ui.label(
            RichText::new("Explore the 8 pure-Rust crates powering crorust cross-compilation and packaging.")
                .size(14.0)
                .weak(),
        );
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        ScrollArea::horizontal()
            .id_salt("crate_pills_scroll")
            .show(ui, |ui| {
            ui.horizontal(|ui| {
                for c in ALL_CRATES {
                    let selected = self.selected_crate_id == c.id;
                    if ui.selectable_label(selected, RichText::new(c.name).strong()).clicked() {
                        self.selected_crate_id = c.id;
                    }
                }
            });
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

            ui.horizontal(|ui| {
                ui.heading(RichText::new(info.name).size(22.0).strong());
                ui.label(RichText::new(format!("— {}", info.title)).size(15.0).weak());
            });

            ui.add_space(8.0);
            ui.label(RichText::new(info.summary).size(14.0));
            ui.add_space(12.0);

            ui.heading(RichText::new("Architectural Responsibilities").size(16.0));
            ui.label(RichText::new(info.details).size(13.5).color(ui.visuals().text_color()));

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Formats Handled:").strong());
                for fmt in info.formats {
                    ui.label(RichText::new(*fmt).monospace().color(Color32::from_rgb(31, 111, 235)));
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Crate Dependencies:").strong());
                for dep in info.dependencies {
                    ui.label(RichText::new(*dep).monospace().weak());
                }
            });

            ui.add_space(16.0);
            ui.heading(RichText::new("Example Usage / CLI Syntax").size(16.0));
            crate::rust_view_ui(ui, info.sample_code);
        });
}
