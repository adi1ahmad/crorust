pub mod hero;
pub mod terminal;
pub mod matrix;
pub mod views;

use egui::{ScrollArea, Ui};
use crate::navigation::NavigationState;

pub fn render_section_divider(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
) {
    ui.add_space(28.0);
    ui.separator();
    ui.add_space(16.0);

    ui.label(
        egui::RichText::new(format!("{icon} {title}"))
            .size(22.0)
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(subtitle)
            .size(13.0)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(16.0);
}

pub const DESKTOP_MAX_WIDTH: f32 = 640.0;
pub const MOBILE_MIN_WIDTH: f32 = 560.0;
pub const MOBILE_BREAKPOINT: f32 = 672.0;

pub fn centered_container(
    ui: &mut egui::Ui,
    desired_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let avail_w = ui.available_width();
    let is_mobile = crate::is_mobile(ui.ctx()) || avail_w < MOBILE_BREAKPOINT;

    let target_w = if is_mobile {
        desired_width.max(MOBILE_MIN_WIDTH)
    } else {
        desired_width.min(DESKTOP_MAX_WIDTH).min(avail_w - 32.0).max(280.0)
    };

    let left_pad = ((avail_w - target_w) / 2.0).max(16.0);

    ui.horizontal(|ui| {
        ui.add_space(left_pad);
        ui.vertical(|ui| {
            ui.set_width(target_w);
            ui.set_max_width(target_w);
            add_contents(ui);
        });
    });
}

pub use centered_container as responsive_container;

#[derive(Default)]
pub struct WebsiteView {
    terminal: terminal::TerminalSimulator,
    crates_view: views::crates::CratesExplorerView,
    commands_view: views::commands::CommandsView,
}

impl WebsiteView {
    pub fn ui(&mut self, ui: &mut Ui, nav: &mut NavigationState) -> egui::scroll_area::ScrollAreaOutput<()> {
        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < MOBILE_BREAKPOINT;

        let scroll_area = if is_mobile {
            ScrollArea::both()
        } else {
            ScrollArea::vertical()
        };

        scroll_area
            .id_salt("site_main_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                ui.add_space(20.0);

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    hero::render_hero(ui, nav);
                });
                ui.add_space(24.0);

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    self.terminal.ui(ui);
                });
                ui.add_space(28.0);

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    matrix::render_compatibility_matrix(ui);
                });

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "📐",
                        "Three-Tier Architecture",
                        "How crorust overcomes the Cross-Compilation Wall using pure-Rust format packagers and hermetic toolchains.",
                    );
                    views::overview::render_overview(ui, nav);
                });

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "💻",
                        "Multi-Host Installation",
                        "Get up and running on Linux, macOS, Windows, or NixOS in less than two minutes.",
                    );
                    views::installation::render_installation(ui);
                });

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "📦",
                        "In-Tree Crate Ecosystem",
                        "Inspect the 8 modular, pure-Rust crates that power crorust packaging and code signing.",
                    );
                    self.crates_view.ui(ui, nav);
                });

                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "💻",
                        "CLI Command Manual",
                        "Interactive workflow command generator and complete flag reference.",
                    );
                    self.commands_view.ui(ui);
                });

                ui.add_space(56.0);
            })
    }
}
