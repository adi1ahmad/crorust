pub mod overview;
pub mod installation;
pub mod crates;
pub mod crate_data;
pub mod commands;
pub mod recipes;

use egui::{RichText, ScrollArea, Ui};
use crate::navigation::{NavigationState, CrorustChapter};
use crate::workspace::WindowManager;

#[derive(Default)]
pub struct CrorustPlatformView {
    search_query: String,
    crates_view: crates::CratesExplorerView,
    commands_view: commands::CommandsView,
}

impl CrorustPlatformView {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        nav: &mut NavigationState,
        _windows: &mut WindowManager,
    ) {
        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 720.0;

        if is_mobile {
            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for chapter in CrorustChapter::ALL {
                        let selected = nav.crorust_chapter == *chapter;
                        if ui.selectable_label(selected, chapter.title()).clicked() {
                            nav.navigate_to_crorust(*chapter);
                        }
                    }
                });
                ui.separator();
                self.render_active_chapter(ui, nav);
            });
        } else {
            egui::Panel::left("crorust_docs_sidebar")
                .resizable(false)
                .default_size(260.0)
                .show(ui, |ui| {
                    self.sidebar_ui(ui, nav);
                });

            egui::CentralPanel::default().show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_active_chapter(ui, nav);
                    });
            });
        }
    }

    fn sidebar_ui(&mut self, ui: &mut Ui, nav: &mut NavigationState) {
        ui.add_space(8.0);
        ui.label(RichText::new("📦 crorust Build Platform").strong().size(16.0));
        ui.label(RichText::new("Cross-Compilation Documentation").small().weak());
        ui.add_space(8.0);

        ui.add(
            egui::TextEdit::singleline(&mut self.search_query)
                .hint_text("🔍 Search documentation...")
                .desired_width(ui.available_width()),
        );
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                for chapter in CrorustChapter::ALL {
                    let title = chapter.title();
                    if !self.search_query.is_empty()
                        && !title.to_lowercase().contains(&self.search_query.to_lowercase())
                    {
                        continue;
                    }

                    let selected = nav.crorust_chapter == *chapter;
                    if ui.selectable_label(selected, RichText::new(title).size(13.0)).clicked() {
                        nav.navigate_to_crorust(*chapter);
                    }
                }
            });
        });
    }

    fn render_active_chapter(
        &mut self,
        ui: &mut Ui,
        nav: &mut NavigationState,
    ) {
        ui.add_space(12.0);

        match nav.crorust_chapter {
            CrorustChapter::Overview => overview::render_overview(ui, nav),
            CrorustChapter::Installation => installation::render_installation(ui),
            CrorustChapter::Crates => self.crates_view.ui(ui, nav),
            CrorustChapter::CliCommands => self.commands_view.ui(ui),
            CrorustChapter::ToolchainsAndSysroots => overview::render_toolchains(ui),
            CrorustChapter::PackagingInternals => overview::render_packaging(ui),
            CrorustChapter::DevicesAndDebugging => overview::render_devices(ui),
        }

        ui.add_space(32.0);
    }
}
