use std::collections::BTreeSet;

use crate::workspace::views::About;
#[cfg(feature = "glow")]
use crate::AppView;
use crate::Demo;
use egui::{ScrollArea, Ui};

struct WindowGroup {
    demos: Vec<Box<dyn Demo>>,
}

impl core::ops::Add for WindowGroup {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut demos = self.demos;
        demos.extend(other.demos);
        Self { demos }
    }
}

impl WindowGroup {
    pub fn new(demos: Vec<Box<dyn Demo>>) -> Self {
        Self { demos }
    }

    pub fn checkboxes(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self { demos } = self;
        for demo in demos {
            if demo.is_enabled(ui.ctx()) {
                let mut is_open = open.contains(demo.name());
                ui.toggle_value(&mut is_open, demo.name());
                set_open(open, demo.name(), is_open);
            }
        }
    }

    pub fn windows(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self { demos } = self;
        for demo in demos {
            let mut is_open = open.contains(demo.name());
            demo.show(ui, &mut is_open);
            set_open(open, demo.name(), is_open);
        }
    }

    pub fn logic(&mut self, ctx: &egui::Context) {
        for demo in &mut self.demos {
            demo.logic(ctx);
        }
    }
}

fn set_open(open: &mut BTreeSet<String>, key: &str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}

pub struct WindowGroups {
    about: About,
    pub ruxt_manual: crate::manual::RuxtManualWindow,
    components: WindowGroup,
    tests: WindowGroup,

    #[cfg(any(feature = "glow", feature = "wgpu"))]
    pub scene_3d: Option<crate::workspace::views::Viewport3D>,
}

impl Default for WindowGroups {
    fn default() -> Self {
        let components_demos: Vec<Box<dyn Demo>> = vec![
            #[cfg(feature = "markdown")]
            Box::<crate::markdown::Editor>::default(),
            #[cfg(feature = "media_viewer")]
            Box::<crate::workspace::views::MediaViewer>::default(),
            Box::<crate::workspace::views::AnalogClock>::default(),
            Box::<crate::workspace::views::widgets::WidgetShowcase>::default(),
            Box::<crate::workspace::views::code_editor::CodeEditor>::default(),
            Box::<crate::workspace::views::code_viewer::CodeExample>::default(),
            Box::<crate::workspace::views::canvas::Painting>::default(),
            Box::<crate::workspace::views::bezier_curves::PaintBezier>::default(),
            Box::<crate::workspace::views::data_tables::TableDemo>::default(),
            Box::<crate::workspace::views::layout_strips::StripDemo>::default(),
            Box::<crate::workspace::views::drag_drop::DragAndDropDemo>::default(),
            Box::<crate::workspace::views::modals::Modals>::default(),
            Box::<crate::workspace::views::popups::PopupsDemo>::default(),
            Box::<crate::workspace::views::scrolling::Scrolling>::default(),
            Box::<crate::workspace::views::sliders::Sliders>::default(),
            Box::<crate::workspace::views::toggle_switch::ToggleSwitch>::default(),
            Box::<crate::workspace::views::undo_redo::UndoRedoDemo>::default(),
            Box::<crate::workspace::views::dancing_strings::DancingStrings>::default(),
        ];

        let tests_demos: Vec<Box<dyn Demo>> = vec![
            #[cfg(feature = "network")]
            Box::<crate::workspace::views::NetworkClient>::default(),
            Box::<crate::workspace::views::DisplayCalibration>::default(),
            Box::<crate::workspace::views::tests::ClipboardTest>::default(),
            Box::<crate::workspace::views::tests::CursorTest>::default(),
            Box::<crate::workspace::views::tests::GridTest>::default(),
            Box::<crate::workspace::views::tests::IdTest>::default(),
            Box::<crate::workspace::views::tests::InputEventHistory>::default(),
            Box::<crate::workspace::views::tests::InputTest>::default(),
            Box::<crate::workspace::views::tests::LayoutTest>::default(),
            Box::<crate::workspace::views::tests::ManualLayoutTest>::default(),
            Box::<crate::workspace::views::tests::SvgTest>::default(),
            Box::<crate::workspace::views::tests::TessellationTest>::default(),
            Box::<crate::workspace::views::tests::WindowResizeTest>::default(),
        ];

        Self {
            about: About::default(),
            ruxt_manual: crate::manual::RuxtManualWindow::default(),
            components: WindowGroup::new(components_demos),
            tests: WindowGroup::new(tests_demos),
            #[cfg(any(feature = "glow", feature = "wgpu"))]
            scene_3d: None,
        }
    }
}

impl WindowGroups {
    pub fn about_checkbox(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let mut is_open = open.contains(self.about.name());
        ui.toggle_value(&mut is_open, self.about.name());
        set_open(open, self.about.name(), is_open);
    }

    pub fn checkboxes(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        ui.label(egui::RichText::new("📖 Living Documentation").strong());
        let mut is_open = open.contains(self.ruxt_manual.name());
        ui.toggle_value(&mut is_open, self.ruxt_manual.name());
        set_open(open, self.ruxt_manual.name(), is_open);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label(egui::RichText::new("📦 Showcase & Components").strong());
        #[cfg(any(feature = "glow", feature = "wgpu"))]
        if let Some(scene_3d) = &mut self.scene_3d {
            let mut is_open = open.contains(scene_3d.name());
            ui.toggle_value(&mut is_open, scene_3d.name());
            set_open(open, scene_3d.name(), is_open);
        }
        self.components.checkboxes(ui, open);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label(egui::RichText::new("🔬 Verification & Tests").strong());
        self.tests.checkboxes(ui, open);
    }

    pub fn windows(&mut self, ui: &mut Ui, open: &mut BTreeSet<String>) {
        let Self {
            about,
            ruxt_manual,
            components,
            tests,
            #[cfg(any(feature = "glow", feature = "wgpu"))]
            scene_3d,
        } = self;
        {
            let mut is_open = open.contains(about.name());
            about.show(ui, &mut is_open);
            set_open(open, about.name(), is_open);
        }
        {
            let mut is_open = open.contains(ruxt_manual.name());
            ruxt_manual.show(ui, &mut is_open);
            set_open(open, ruxt_manual.name(), is_open);
        }
        #[cfg(any(feature = "glow", feature = "wgpu"))]
        if let Some(scene_3d) = scene_3d {
            let mut is_open = open.contains(scene_3d.name());
            scene_3d.show(ui, &mut is_open);
            set_open(open, scene_3d.name(), is_open);
        }
        components.windows(ui, open);
        tests.windows(ui, open);
    }

    pub fn logic(&mut self, ctx: &egui::Context) {
        self.components.logic(ctx);
        self.tests.logic(ctx);
    }

    #[cfg(feature = "glow")]
    pub fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        #[cfg(any(feature = "glow", feature = "wgpu"))]
        if let Some(scene_3d) = &mut self.scene_3d {
            scene_3d.on_exit(gl);
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct WindowManager {
    #[cfg_attr(feature = "serde", serde(skip))]
    groups: WindowGroups,

    open: BTreeSet<String>,
    sidebar_open: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub has_initialized_layout: bool,
}

pub type DemoWindows = WindowManager;

impl Default for WindowManager {
    fn default() -> Self {
        let mut open = BTreeSet::new();

        // Default to Ruxt Framework Manual as the primary window
        set_open(&mut open, "📖 Ruxt Framework Manual", true);

        Self {
            groups: Default::default(),
            open,
            sidebar_open: false,
            has_initialized_layout: false,
        }
    }
}

impl WindowManager {
    pub fn is_window_open(&self, name: &str) -> bool {
        self.open.contains(name)
    }

    pub fn set_window_open(&mut self, name: &str, is_open: bool) {
        set_open(&mut self.open, name, is_open);
    }

    pub fn open_window(&mut self, name: &str) {
        self.set_window_open(name, true);
    }

    pub fn open_and_focus(&mut self, ctx: &egui::Context, name: &str) {
        self.set_window_open(name, true);
        ctx.request_repaint();
    }

    pub fn all_window_names(&self) -> Vec<&'static str> {
        vec![
            "About crorust.ruxt",
            "📖 Ruxt Framework Manual",
            "📝 Markdown Editor",
            "🖼 Media Viewer",
            "⏰ Analog Clock",
            "📦 Widget Showcase",
            "💻 Code Editor",
            "💻 Code Example",
            "🎨 Painting",
            "📈 Bézier Curve",
            "📊 Data Tables",
            "📏 Strip Layout Demo",
            "📦 Drag and Drop",
            "💬 Modals",
            "💭 Popups",
            "📜 Scrolling",
            "📏 Sliders",
            "🔘 Toggle Switch",
            "↩ Undo and Redo",
            "✨ Dancing Strings",
            "🎲 3D Viewport",
            "🌐 Network Client",
            "🎨 Display Calibration",
            "Clipboard Test",
            "Cursor Test",
            "Grid Test",
            "Id Test",
            "Input Event History",
            "Input Test",
            "Layout Test",
            "Manual Layout Test",
            "SVG Test",
            "Tessellation Test",
            "Window Resize Test",
        ]
    }

    pub fn close_all(&mut self) {
        self.open.clear();
    }

    pub fn toggle_menu_item(&mut self, ui: &mut egui::Ui, name: &'static str) {
        let mut is_open = self.is_window_open(name);
        if ui.checkbox(&mut is_open, name).changed() {
            self.set_window_open(name, is_open);
        }
    }

    pub fn sidebar_open(&self) -> bool {
        self.sidebar_open
    }

    pub fn set_sidebar_open(&mut self, open: bool) {
        self.sidebar_open = open;
    }

    #[cfg(any(feature = "glow", feature = "wgpu"))]
    pub fn set_scene_3d(&mut self, scene_3d: Option<crate::workspace::views::Viewport3D>) {
        self.groups.scene_3d = scene_3d;
    }

    #[cfg(feature = "glow")]
    pub fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.groups.on_exit(gl);
    }

    /// Adapts the initial workspace configuration to screen dimensions on first frame.
    pub fn ensure_responsive_initialization(&mut self, ctx: &egui::Context) {
        if self.has_initialized_layout {
            return;
        }
        self.has_initialized_layout = true;

        let is_mobile = crate::is_mobile(ctx);
        if is_mobile {
            // MOBILE: Strict minimal distraction-free setup
            // 1. Sidebar is closed on entry
            self.sidebar_open = false;

            // 2. ONLY the Ruxt Framework Manual is open
            self.open.clear();
            set_open(&mut self.open, "📖 Ruxt Framework Manual", true);
        } else {
            // DESKTOP: Living manual open alongside sidebar
            self.sidebar_open = true;
            set_open(&mut self.open, "📖 Ruxt Framework Manual", true);
            set_open(&mut self.open, "About crorust.ruxt", true);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.ensure_responsive_initialization(ui.ctx());
        self.desktop_ui(ui);
    }

    pub fn logic(&mut self, ctx: &egui::Context) {
        self.groups.logic(ctx);
    }

    fn desktop_ui(&mut self, ui: &mut egui::Ui) {
        let is_mobile = crate::is_mobile(ui.ctx());

        self.groups.windows(ui, &mut self.open);

        if let Some(target) = self.groups.ruxt_manual.target_to_launch.take() {
            self.open_and_focus(ui.ctx(), target);
            if is_mobile {
                self.sidebar_open = false;
            }
        }

        if self.sidebar_open {
            let mut collapse = false;
            let central_rect = ui.max_rect();

            let default_width = if is_mobile {
                (central_rect.width() * 0.85).clamp(200.0, 320.0)
            } else {
                260.0
            };

            let panel_id = egui::Id::new("workspace_sidebar_overlay");
            let width: f32 = ui.data_mut(|d| *d.get_temp_mut_or(panel_id, default_width));

            let panel_pos = egui::pos2(
                (central_rect.right() - width).max(central_rect.left()),
                central_rect.top(),
            );
            let panel_rect = egui::Rect::from_min_size(
                panel_pos,
                egui::vec2(width.min(central_rect.width()), central_rect.height()),
            );

            egui::Area::new(panel_id)
                .order(egui::Order::Foreground)
                .fixed_pos(panel_rect.min)
                .show(ui.ctx(), |ui| {
                    let frame = egui::Frame::side_top_panel(ui.style())
                        .fill(ui.visuals().panel_fill)
                        .stroke(ui.visuals().window_stroke)
                        .shadow(ui.visuals().window_shadow);

                    frame.show(ui, |ui| {
                        ui.set_min_size(panel_rect.size());
                        ui.set_max_size(panel_rect.size());

                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Image::new(egui::include_image!("../../assets/ruxt-logo.svg"))
                                    .max_height(24.0)
                                    .tint(ui.visuals().strong_text_color()),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .small_button("⏵")
                                    .on_hover_text("Collapse Workspace Sidebar (Ctrl+B)")
                                    .clicked()
                                {
                                    collapse = true;
                                }

                                use egui::special_emojis::GITHUB;
                                ui.hyperlink_to(
                                    egui::RichText::new(GITHUB).size(16.0),
                                    "https://github.com/adi1ahmad/crorust",
                                );
                            });
                        });

                        let prev_open_count = self.open.len();

                        ui.add_space(4.0);
                        self.groups.about_checkbox(ui, &mut self.open);

                        ui.add_space(4.0);
                        ui.separator();

                        self.window_list_ui(ui);

                        // On mobile: auto-close drawer once a window has been opened
                        if is_mobile && self.open.len() > prev_open_count {
                            collapse = true;
                        }
                    });
                });

            if collapse {
                self.sidebar_open = false;
            }
        }
    }

    fn window_list_ui(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("📐 Organize")
                        .on_hover_text("Auto-tile and arrange open windows (Ctrl+Shift+O)")
                        .clicked()
                    {
                        ui.memory_mut(|mem| mem.reset_areas());
                    }
                    if ui
                        .button("✕ Close All")
                        .on_hover_text("Close all floating windows")
                        .clicked()
                    {
                        self.close_all();
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                self.groups.checkboxes(ui, &mut self.open);
                ui.add_space(8.0);
            });
        });
    }
}
