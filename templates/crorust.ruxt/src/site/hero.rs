use egui::{Color32, RichText, Ui, vec2};
use crate::navigation::{NavigationState, AppMode};

pub fn render_hero(ui: &mut Ui, nav: &mut NavigationState) {
    ui.set_width(ui.available_width());
    ui.add(
        egui::Image::new(egui::include_image!("../../assets/ruxt-logo.svg"))
            .max_height(48.0)
            .tint(ui.visuals().strong_text_color()),
    );
    ui.add_space(10.0);

    ui.heading(
        RichText::new("crorust & ruxt")
            .size(34.0)
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.add_space(6.0);

    ui.label(
        RichText::new("Hermetic Multi-Host Cross-Compilation Platform & Pure-Rust GUI Framework")
            .size(17.0)
            .color(ui.visuals().text_color()),
    );
    ui.add_space(8.0);

    ui.label(
        RichText::new(
            "Compile, package, and cryptographically sign native applications for Android, iOS, \
            macOS, Windows, Linux, and WebAssembly from Linux, macOS, or Windows — zero external IDEs required.",
        )
        .size(13.5)
        .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(18.0);

    // Call to Action Buttons
    let is_mobile = ui.available_width() < 520.0;
    if is_mobile {
        ui.vertical_centered(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("🚀 Explore crorust Platform").size(14.5).strong().color(Color32::WHITE))
                        .fill(Color32::from_rgb(31, 111, 235))
                        .min_size(vec2(ui.available_width() - 32.0, 40.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Home);
            }
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("💻 Launch Workspace & Manual").size(14.5).strong())
                        .min_size(vec2(ui.available_width() - 32.0, 40.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Workspace);
            }
        });
    } else {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("🚀 Explore crorust Platform").size(14.5).strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(31, 111, 235))
                            .min_size(vec2(210.0, 38.0)),
                    )
                    .clicked()
                {
                    nav.navigate_to_mode(AppMode::Home);
                }
                ui.add_space(16.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("💻 Launch Workspace & Manual").size(14.5).strong())
                            .min_size(vec2(240.0, 38.0)),
                    )
                    .clicked()
                {
                    nav.navigate_to_mode(AppMode::Workspace);
                }
            });
        });
    }
}

