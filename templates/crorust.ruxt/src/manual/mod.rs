pub mod chapter;
pub mod content;
pub mod widgets;

use std::collections::BTreeSet;

use chapter::RuxtTutorialChapter;
use egui::{Color32, RichText, ScrollArea, Ui, Vec2};
use widgets::ManualPlaygroundState;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RuxtManualWindow {
    /// Active chapter currently selected for viewing.
    pub selected_chapter: RuxtTutorialChapter,
    /// Instant search filter query for filtering chapters and topics.
    pub search_query: String,
    /// Set of chapter numbers (1..=7) marked as completed by the reader.
    pub completed_chapters: BTreeSet<usize>,
    /// Whether the 240px Table of Contents sidebar is open on desktop.
    pub sidebar_expanded: bool,
    /// Internal interactive playground state for chapter labs.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub playground: ManualPlaygroundState,
    /// Target view or demo to launch if requested by a chapter action.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub target_to_launch: Option<&'static str>,
}

impl Default for RuxtManualWindow {
    fn default() -> Self {
        Self {
            selected_chapter: RuxtTutorialChapter::Windowing,
            search_query: String::new(),
            completed_chapters: BTreeSet::new(),
            sidebar_expanded: true,
            playground: ManualPlaygroundState::default(),
            target_to_launch: None,
        }
    }
}

impl RuxtManualWindow {
    pub fn name(&self) -> &'static str {
        "📖 Ruxt Framework Manual"
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 550.0;

        if is_mobile {
            self.render_mobile(ui);
        } else {
            self.render_desktop(ui);
        }
    }

    fn render_desktop(&mut self, ui: &mut Ui) {
        let avail_h = ui.available_height();
        ui.horizontal(|ui| {
            ui.set_min_height(avail_h);
            // 1. Collapsible 240px TOC Sidebar
            if self.sidebar_expanded {
                ui.allocate_ui_with_layout(
                    Vec2::new(240.0, avail_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.render_toc_sidebar(ui);
                    },
                );
                ui.separator();
            }

            // 2. Main Documentation Viewport
            ui.vertical(|ui| {
                ui.set_min_height(avail_h);
                self.render_breadcrumbs(ui);
                ui.separator();

                let output = ScrollArea::vertical()
                    .id_salt("manual_desktop_content_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        self.render_selected_chapter(ui);
                        self.render_navigation_footer(ui);
                        ui.add_space(16.0);
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
            });
        });
    }

    fn render_mobile(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // Mobile compact chapter selector pills
            ui.horizontal_wrapped(|ui| {
                for chapter in RuxtTutorialChapter::ALL {
                    let is_selected = self.selected_chapter == *chapter;
                    let is_completed = self.completed_chapters.contains(&chapter.number());
                    let label = if is_completed {
                        format!("✓ {}", chapter.pill_title())
                    } else {
                        chapter.pill_title().to_string()
                    };

                    if ui
                        .selectable_label(is_selected, RichText::new(label).small().strong())
                        .clicked()
                    {
                        self.selected_chapter = *chapter;
                    }
                }
            });

            ui.add_space(6.0);
            self.render_breadcrumbs(ui);
            ui.separator();

            let avail_w = ui.available_width();
            let output = ScrollArea::vertical()
                .id_salt("manual_mobile_content_scroll")
                .auto_shrink([false, false])
                .max_width(avail_w)
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    self.render_selected_chapter(ui);
                    self.render_navigation_footer(ui);
                    ui.add_space(16.0);
                });

            crate::ui::scroll::assist_touch_scroll(
                ui.ctx(),
                output.id,
                None,
                output.inner_rect,
                output.content_size,
            );
        });
    }

    fn render_toc_sidebar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("📖 Table of Contents").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("⏴")
                    .on_hover_text("Collapse Sidebar")
                    .clicked()
                {
                    self.sidebar_expanded = false;
                }
            });
        });

        ui.add_space(6.0);

        // Instant keyword search
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("🔍 Search topics...")
                    .desired_width(ui.available_width() - 24.0),
            );
            if !self.search_query.is_empty() && ui.small_button("✖").clicked() {
                self.search_query.clear();
            }
        });

        ui.add_space(6.0);

        // Reading progress indicator
        let total_chapters = RuxtTutorialChapter::ALL.len();
        let completed_count = self.completed_chapters.len();
        let progress_frac = if total_chapters > 0 {
            completed_count as f32 / total_chapters as f32
        } else {
            0.0
        };

        ui.add(
            egui::ProgressBar::new(progress_frac)
                .text(format!(
                    "{completed_count}/{total_chapters} Read ({:.0}%)",
                    progress_frac * 100.0
                ))
                .animate(false),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Chapter list with search filter
        let query = self.search_query.trim().to_lowercase();
        for chapter in RuxtTutorialChapter::ALL {
            let is_selected = self.selected_chapter == *chapter;
            let is_completed = self.completed_chapters.contains(&chapter.number());

            let matches_query = query.is_empty()
                || chapter.full_title().to_lowercase().contains(&query)
                || chapter.summary().to_lowercase().contains(&query)
                || chapter.subsections().iter().any(|s| {
                    s.title.to_lowercase().contains(&query)
                        || s.summary.to_lowercase().contains(&query)
                });

            if !matches_query {
                continue;
            }

            let status_badge = if is_completed { "✓ " } else { "" };
            let row_text = format!(
                "{}{}. {}",
                status_badge,
                chapter.number(),
                chapter
                    .pill_title()
                    .split(". ")
                    .nth(1)
                    .unwrap_or(chapter.pill_title())
            );

            let row_response = ui.selectable_label(
                is_selected,
                RichText::new(&row_text).strong().color(if is_selected {
                    ui.visuals().strong_text_color()
                } else if is_completed {
                    Color32::from_rgb(46, 160, 67)
                } else {
                    ui.visuals().text_color()
                }),
            );

            if row_response.clicked() {
                self.selected_chapter = *chapter;
            }

            if is_selected || !query.is_empty() {
                ui.indent(chapter.number(), |ui| {
                    for sub in chapter.subsections() {
                        let sub_matches = query.is_empty()
                            || sub.title.to_lowercase().contains(&query)
                            || sub.summary.to_lowercase().contains(&query);

                        if !sub_matches {
                            continue;
                        }

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(sub.id).small().weak());
                            ui.label(RichText::new(sub.title).small());
                        });
                    }
                });
            }
            ui.add_space(2.0);
        }
    }

    fn render_breadcrumbs(&mut self, ui: &mut Ui) {
        let is_mobile = crate::is_mobile(ui.ctx());

        ui.horizontal(|ui| {
            if !self.sidebar_expanded && !is_mobile {
                if ui
                    .small_button("☰ Topics")
                    .on_hover_text("Open Table of Contents")
                    .clicked()
                {
                    self.sidebar_expanded = true;
                }
                ui.separator();
            }

            ui.label(RichText::new("📖 Manual").weak());
            ui.label(RichText::new("›").weak());
            ui.label(
                RichText::new(format!("Chapter {}", self.selected_chapter.number()))
                    .strong()
                    .color(ui.visuals().strong_text_color()),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ch_num = self.selected_chapter.number();
                let is_completed = self.completed_chapters.contains(&ch_num);

                if is_completed {
                    if ui
                        .button(
                            RichText::new("✓ Completed")
                                .color(Color32::from_rgb(46, 160, 67))
                                .small(),
                        )
                        .on_hover_text("Click to mark as unread")
                        .clicked()
                    {
                        self.completed_chapters.remove(&ch_num);
                    }
                } else if ui
                    .button(RichText::new("Mark Complete").small())
                    .on_hover_text("Mark this chapter as completed")
                    .clicked()
                {
                    self.completed_chapters.insert(ch_num);
                }

                ui.label(
                    RichText::new(format!(
                        "⏱ {} min read",
                        self.selected_chapter.read_time_minutes()
                    ))
                    .small()
                    .weak(),
                );
            });
        });
    }

    fn render_selected_chapter(&mut self, ui: &mut Ui) {
        match self.selected_chapter {
            RuxtTutorialChapter::Windowing => {
                content::ch01_windowing::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::WinitBridge => {
                content::ch02_winit_bridge::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::ImguiParadigm => {
                content::ch03_imgui_paradigm::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::ContextInternals => {
                content::ch04_context_internals::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::LayoutGeometry => {
                content::ch05_layout_geometry::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::GpuPipeline => {
                content::ch06_gpu_pipeline::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
            RuxtTutorialChapter::RuxtJourney => {
                content::ch07_ruxt_journey::render_chapter(
                    ui,
                    &mut self.playground,
                    &mut self.target_to_launch,
                );
            }
        }
    }

    fn render_navigation_footer(&mut self, ui: &mut Ui) {
        ui.add_space(24.0);
        ui.separator();
        ui.add_space(12.0);

        let prev = self.selected_chapter.prev();
        let next = self.selected_chapter.next();
        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 550.0;

        if is_mobile {
            // Mobile: stack navigation cards vertically for comfortable thumb targets and zero overflow
            ui.vertical(|ui| {
                if let Some(next_ch) = next {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .corner_radius(6.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(format!("Next Chapter (Ch {}) ➔", next_ch.number()))
                                    .small()
                                    .weak(),
                            );
                            if ui
                                .button(RichText::new(next_ch.full_title()).strong())
                                .clicked()
                            {
                                self.completed_chapters
                                    .insert(self.selected_chapter.number());
                                self.selected_chapter = next_ch;
                            }
                            ui.add(
                                egui::Label::new(RichText::new(next_ch.summary()).small().weak())
                                    .wrap(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "⏱ {} min read",
                                    next_ch.read_time_minutes()
                                ))
                                .size(10.0)
                                .weak(),
                            );
                        });
                    ui.add_space(8.0);
                }

                if let Some(prev_ch) = prev {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .corner_radius(6.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(format!(
                                    "⬅ Previous Chapter (Ch {})",
                                    prev_ch.number()
                                ))
                                .small()
                                .weak(),
                            );
                            if ui
                                .button(RichText::new(prev_ch.full_title()).strong())
                                .clicked()
                            {
                                self.selected_chapter = prev_ch;
                            }
                            ui.add(
                                egui::Label::new(RichText::new(prev_ch.summary()).small().weak())
                                    .wrap(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "⏱ {} min read",
                                    prev_ch.read_time_minutes()
                                ))
                                .size(10.0)
                                .weak(),
                            );
                        });
                }
            });
        } else {
            let half_width = (ui.available_width() - 16.0) / 2.0;

            ui.horizontal(|ui| {
                // Previous chapter card
                if let Some(prev_ch) = prev {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(half_width.max(120.0));
                            ui.label(
                                RichText::new(format!("⬅ Previous (Ch {})", prev_ch.number()))
                                    .small()
                                    .weak(),
                            );
                            if ui
                                .button(RichText::new(prev_ch.pill_title()).strong())
                                .clicked()
                            {
                                self.selected_chapter = prev_ch;
                            }
                            ui.add(
                                egui::Label::new(RichText::new(prev_ch.summary()).small().weak())
                                    .wrap(),
                            );
                            ui.label(
                                RichText::new(format!("⏱ {} min", prev_ch.read_time_minutes()))
                                    .size(10.0)
                                    .weak(),
                            );
                        });
                } else {
                    ui.allocate_space(Vec2::new(half_width.max(120.0), 10.0));
                }

                // Next chapter card
                if let Some(next_ch) = next {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(half_width.max(120.0));
                            ui.label(
                                RichText::new(format!("Next (Ch {}) ➔", next_ch.number()))
                                    .small()
                                    .weak(),
                            );
                            if ui
                                .button(RichText::new(next_ch.pill_title()).strong())
                                .clicked()
                            {
                                self.completed_chapters
                                    .insert(self.selected_chapter.number());
                                self.selected_chapter = next_ch;
                            }
                            ui.add(
                                egui::Label::new(RichText::new(next_ch.summary()).small().weak())
                                    .wrap(),
                            );
                            ui.label(
                                RichText::new(format!("⏱ {} min", next_ch.read_time_minutes()))
                                    .size(10.0)
                                    .weak(),
                            );
                        });
                }
            });
        }

        ui.add_space(16.0);
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
            window = window.default_size([960.0, 680.0]);
        }

        window.show(ui.ctx(), |ui| {
            self.ui(ui);
        });
    }
}
