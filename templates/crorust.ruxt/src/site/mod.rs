pub mod hero;
pub mod matrix;
pub mod terminal;
pub mod views;

use crate::navigation::NavigationState;
use egui::{ScrollArea, Ui};

pub fn render_section_divider(ui: &mut egui::Ui, icon: &str, title: &str, subtitle: &str) {
    ui.add_space(28.0);
    ui.separator();
    ui.add_space(16.0);

    ui.add(
        egui::Label::new(
            egui::RichText::new(format!("{icon} {title}"))
                .size(22.0)
                .strong()
                .color(ui.visuals().strong_text_color()),
        )
        .wrap(),
    );
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(subtitle)
                .size(13.0)
                .color(ui.visuals().weak_text_color()),
        )
        .wrap(),
    );
    ui.add_space(16.0);
}

pub const DESKTOP_MAX_WIDTH: f32 = 1080.0;

pub fn centered_container(
    ui: &mut egui::Ui,
    desired_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let avail_w = ui.available_width();
    let is_compact = avail_w < 500.0 || crate::is_mobile(ui.ctx());

    if is_compact {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 0))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                add_contents(ui);
            });
    } else {
        let target_w = desired_width
            .min(DESKTOP_MAX_WIDTH)
            .min(avail_w - 48.0)
            .max(320.0);
        let left_pad = ((avail_w - target_w) / 2.0).max(16.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_space(left_pad);
            ui.vertical(|ui| {
                ui.set_width(target_w);
                ui.set_max_width(target_w);
                add_contents(ui);
            });
        });
    }
}

pub use centered_container as responsive_container;

#[derive(Default)]
pub struct WebsiteView {
    workbench: terminal::TerminalWorkbench,
    crates_view: views::crates::CratesExplorerView,
    commands_view: views::commands::CommandsView,
}

impl WebsiteView {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        nav: &mut NavigationState,
    ) -> egui::scroll_area::ScrollAreaOutput<()> {
        let mut scroll = ScrollArea::vertical()
            .id_salt("site_main_scroll")
            .auto_shrink([false, false]);

        if nav.pending_hash_sync {
            let target_y = match nav.crorust_chapter {
                crate::navigation::CrorustChapter::Overview => 0.0,
                crate::navigation::CrorustChapter::Installation => 200.0,
                crate::navigation::CrorustChapter::ToolchainsAndSysroots => 550.0,
                crate::navigation::CrorustChapter::CliCommands => 1400.0,
                crate::navigation::CrorustChapter::Crates => 2100.0,
                crate::navigation::CrorustChapter::PackagingInternals => 2600.0,
                crate::navigation::CrorustChapter::DevicesAndDebugging => 3000.0,
            };
            scroll = scroll.vertical_scroll_offset(target_y);
            nav.pending_hash_sync = false;
        }

        let output = scroll.show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                ui.add_space(16.0);

                // 1. Hero Section: Headline, Target Chips, and Quickstart CTA
                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    hero::render_hero(ui, nav);
                });
                ui.add_space(28.0);

                // 2. Why crorust: Toolchain Comparison & Compatibility Matrix
                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "⚡",
                        "Why crorust: Breaking the Cross-Compilation Wall",
                        "How crorust eliminates 30GB of external platform IDEs with in-tree pure-Rust container bundlers.",
                    );
                    matrix::render_compatibility_matrix(ui);
                });
                ui.add_space(28.0);

                // 3. Cross-Compilation Workbench (Unified Terminal, Recipes, CI & Specs)
                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "💻",
                        "Cross-Compilation Workbench",
                        "Select your host machine and target platform to synthesize instant CLI commands, live simulated build logs, CI matrix workflows, and manifest configs.",
                    );
                    self.workbench.ui(ui);
                });
                ui.add_space(28.0);

                // 4. In-Tree Crates Architecture
                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "📦",
                        "In-Tree Crates Architecture",
                        "Inspect the 8 modular pure-Rust crates that power crorust packaging, AXML compilation, and cryptographic signing.",
                    );
                    self.crates_view.ui(ui, nav);
                });
                ui.add_space(28.0);

                // 5. CLI Command Manual
                centered_container(ui, DESKTOP_MAX_WIDTH, |ui| {
                    render_section_divider(
                        ui,
                        "🛠",
                        "CLI Command Reference",
                        "Complete reference of all crorust flags, subcommands, and target options.",
                    );
                    self.commands_view.ui(ui);
                });

                ui.add_space(64.0);
            });

        if crate::is_mobile(ui.ctx()) || ui.ctx().input(|i| i.has_touch_screen()) {
            crate::ui::scroll::assist_touch_scroll(
                ui.ctx(),
                output.id,
                None,
                output.inner_rect,
                output.content_size,
            );
        }

        output
    }
}
