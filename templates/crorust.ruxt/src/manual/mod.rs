pub mod chapter;
pub mod content;
pub mod widgets;

use egui::{RichText, ScrollArea, Ui};
use chapter::RuxtTutorialChapter;
use widgets::ManualPlaygroundState;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RuxtManualWindow {
    pub selected_chapter: RuxtTutorialChapter,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub playground: ManualPlaygroundState,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub target_to_launch: Option<&'static str>,
}

impl RuxtManualWindow {
    pub fn name(&self) -> &'static str {
        "📖 Ruxt Framework Manual"
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        // 1. Chapter selection pills
        ui.horizontal_wrapped(|ui| {
            for chapter in RuxtTutorialChapter::ALL {
                let selected = self.selected_chapter == *chapter;
                if ui
                    .selectable_label(selected, RichText::new(chapter.pill_title()).small().strong())
                    .clicked()
                {
                    self.selected_chapter = *chapter;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();

        // 2. Main content scroll area
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                match self.selected_chapter {
                    RuxtTutorialChapter::MentalModel => {
                        content::ch01_welcome::render_chapter(ui, &mut self.playground);
                    }
                    RuxtTutorialChapter::Widgets => {
                        content::ch02_widgets::render_chapter(ui, &mut self.playground, &mut self.target_to_launch);
                    }
                    RuxtTutorialChapter::Layout => {
                        content::ch03_layout::render_chapter(ui, &mut self.playground, &mut self.target_to_launch);
                    }
                    RuxtTutorialChapter::Windows => {
                        content::ch04_windows::render_chapter(ui, &mut self.target_to_launch);
                    }
                    RuxtTutorialChapter::Graphics => {
                        content::ch05_graphics::render_chapter(ui, &mut self.playground, &mut self.target_to_launch);
                    }
                    RuxtTutorialChapter::Architecture => {
                        content::ch06_architecture::render_chapter(ui, &mut self.target_to_launch);
                    }
                }

                // 3. Chapter navigation footer (Previous / Next)
                ui.add_space(24.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if let Some(prev) = self.selected_chapter.prev() {
                        if ui.button(format!("⬅ Previous: {}", prev.pill_title())).clicked() {
                            self.selected_chapter = prev;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(next) = self.selected_chapter.next() {
                            if ui.button(RichText::new(format!("Next: {} ➔", next.pill_title())).strong()).clicked() {
                                self.selected_chapter = next;
                            }
                        }
                    });
                });

                ui.add_space(16.0);
            });
    }
}

impl crate::Demo for RuxtManualWindow {
    fn name(&self) -> &'static str {
        "📖 Ruxt Framework Manual"
    }

    fn show(&mut self, ui: &mut Ui, open: &mut bool) {
        let is_mobile = crate::is_mobile(ui.ctx());
        let mut window = egui::Window::new(self.name())
            .open(open)
            .vscroll(false)
            .resizable(!is_mobile);

        if is_mobile {
            let screen = ui.ctx().content_rect();
            window = window
                .fixed_pos([screen.min.x + 8.0, screen.min.y + 42.0])
                .fixed_size([screen.width() - 16.0, screen.height() - 54.0]);
        } else {
            window = window.default_size([720.0, 560.0]);
        }

        window.show(ui.ctx(), |ui| {
            self.ui(ui);
        });
    }
}
