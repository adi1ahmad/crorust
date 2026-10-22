use crate::workspace::WindowManager;

#[cfg(feature = "glow")]
use eframe::glow;

use crate::AppView;

impl AppView for WindowManager {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }

    fn logic(&mut self, ctx: &egui::Context) {
        self.logic(ctx);
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct State {
    pub windows: WindowManager,
    pub settings_panel: crate::ui::SettingsPanel,
    pub navigation: crate::navigation::NavigationState,
}

pub struct RuxtApp {
    pub state: State,

    #[cfg(any(feature = "glow", feature = "wgpu"))]
    scene_3d: Option<crate::workspace::views::Viewport3D>,

    pub website_view: crate::site::WebsiteView,
    pub mobile_drawer_open: bool,
}

impl RuxtApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        #[cfg(feature = "accessibility")]
        cc.egui_ctx
            .add_plugin(crate::ui::accessibility::AccessibilityPlugin::default());

        #[cfg(any(feature = "glow", feature = "wgpu"))]
        let scene_3d = crate::workspace::views::Viewport3D::new(cc);

        #[allow(unused_mut)]
        let mut slf = Self {
            state: State::default(),

            #[cfg(any(feature = "glow", feature = "wgpu"))]
            scene_3d,

            website_view: crate::site::WebsiteView::default(),
            mobile_drawer_open: false,
        };

        #[cfg(feature = "persistence")]
        if let Some(storage) = cc.storage
            && let Some(state) = eframe::get_value(storage, eframe::APP_KEY)
        {
            slf.state = state;
        }

        // Strictly enforce that launching the app opens in Home & crorust Platform mode
        slf.state.navigation.mode = crate::navigation::AppMode::Home;

        #[cfg(any(feature = "glow", feature = "wgpu"))]
        slf.state.windows.set_scene_3d(slf.scene_3d.take());

        slf
    }

    fn top_navigation_bar(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        egui::Panel::top("top_navigation_bar").show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::menu::MenuBar::new().ui(ui, |ui| {
                        let is_mobile = crate::is_mobile(ui.ctx()) || ui.available_width() < 600.0;

                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Image::new(egui::include_image!("../assets/ruxt-logo.svg"))
                                    .max_height(20.0)
                                    .tint(ui.visuals().strong_text_color()),
                            )
                            .on_hover_text("crorust & ruxt — https://crorust.com");

                            ui.separator();

                            if is_mobile {
                                if ui
                                    .selectable_label(self.mobile_drawer_open, "☰ Menu")
                                    .on_hover_text("Open navigation menu")
                                    .clicked()
                                {
                                    self.mobile_drawer_open = !self.mobile_drawer_open;
                                }
                            } else {
                                self.render_mode_tabs(ui);
                            }

                            if self.state.navigation.mode == crate::navigation::AppMode::Workspace {
                                ui.separator();
                            
                                self.view_menu(ui);
                            }
                            self.help_menu(ui);
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.state.navigation.mode == crate::navigation::AppMode::Workspace {
                                let sidebar_open = self.state.windows.sidebar_open();
                                let is_compact = crate::is_mobile(ui.ctx()) || ui.available_width() < 420.0;
                                let sidebar_label = if is_compact { "📦" } else { "📦 Windows" };
                                if ui
                                    .selectable_label(sidebar_open, sidebar_label)
                                    .on_hover_text("Toggle Workspace Sidebar (Ctrl+B)")
                                    .clicked()
                                {
                                    self.state.windows.set_sidebar_open(!sidebar_open);
                                }

                                ui.separator();
                            }

                            self.state.settings_panel.end_of_top_bar(ui, frame);

                            ui.separator();

                            egui::widgets::global_theme_preference_switch(ui);

                            ui.separator();

                            let fps = self.state.settings_panel.fps();
                            ui.label(egui::RichText::new(format!("{fps:.0} FPS")).small().color(
                                if fps >= 55.0 {
                                    egui::Color32::from_rgb(120, 210, 120)
                                } else {
                                    egui::Color32::from_rgb(230, 180, 80)
                                },
                            ))
                            .on_hover_text("Real-time GPU frame presentation frequency");
                        });
                    });
                });
        });

        if self.mobile_drawer_open {
            self.render_mobile_drawer(ui);
        }
    }

    fn render_mode_tabs(&mut self, ui: &mut egui::Ui) {
        use crate::navigation::AppMode;

        let current = self.state.navigation.mode;

        if ui
            .selectable_label(current == AppMode::Home, "🏠 Home Platform")
            .on_hover_text("crorust build platform, cross-compilation matrix & documentation")
            .clicked()
        {
            self.state.navigation.navigate_to_mode(AppMode::Home);
        }

        if ui
            .selectable_label(current == AppMode::Workspace, "💻 Template Showcase")
            .on_hover_text("Interactive multi-window workspace with live widgets & ruxt framework manual")
            .clicked()
        {
            self.state.navigation.navigate_to_mode(AppMode::Workspace);
        }
    }

    fn render_mobile_drawer(&mut self, ui: &mut egui::Ui) {
        use crate::navigation::AppMode;

        egui::Panel::left("mobile_drawer_panel")
            .resizable(false)
            .default_size(260.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading("Navigation");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            self.mobile_drawer_open = false;
                        }
                    });
                });
                ui.separator();
                ui.add_space(8.0);

                if ui.button("🏠 Home & crorust Platform").clicked() {
                    self.state.navigation.navigate_to_mode(AppMode::Home);
                    self.mobile_drawer_open = false;
                }
                if ui.button("💻 Workspace & ruxt Manual").clicked() {
                    self.state.navigation.navigate_to_mode(AppMode::Workspace);
                    self.mobile_drawer_open = false;
                }

                ui.add_space(16.0);
                ui.separator();
                ui.label(egui::RichText::new("External Links").small().weak());
                ui.hyperlink_to("GitHub Repository", "https://github.com/adi1ahmad/crorust");
                ui.hyperlink_to("Report Issue", "https://github.com/adi1ahmad/crorust/issues");
            });
    }

  
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            
            ui.set_min_width(220.0);

            let organize_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::O,
            );
            if ui
                .add(
                    egui::Button::new("Organize Windows")
                        .shortcut_text(ui.ctx().format_shortcut(&organize_shortcut)),
                )
                .clicked()
            {
                ui.memory_mut(|mem| mem.reset_areas());
                ui.close();
            }

            let reset_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::R,
            );
            if ui
                .add(
                    egui::Button::new("Reset Window Layout")
                        .shortcut_text(ui.ctx().format_shortcut(&reset_shortcut)),
                )
                .on_hover_text("Forget all custom window sizes, offsets, and scroll states")
                .clicked()
            {
                ui.memory_mut(|mem| *mem = Default::default());
                ui.close();
            }

            ui.separator();

            if ui.button("Close All Windows").clicked() {
                self.state.windows.close_all();
                ui.close();
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.separator();

                let is_fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                if ui
                    .add(
                        egui::Button::new(if is_fullscreen {
                            "Exit Fullscreen"
                        } else {
                            "Fullscreen"
                        })
                        .shortcut_text("F11"),
                    )
                    .clicked()
                {
                    ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                    ui.close();
                }

                ui.separator();

                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
   
          

            let mut sidebar_open = self.state.windows.sidebar_open();
            if ui
                .checkbox(&mut sidebar_open, "Workspace Panel (Ctrl+B)")
                .changed()
            {
                self.state.windows.set_sidebar_open(sidebar_open);
            }

            if ui
                .checkbox(&mut self.state.settings_panel.open, "Settings Panel")
                .changed()
            {
                ui.close();
            }

            ui.separator();

            let ppp = ui.ctx().pixels_per_point();
            ui.horizontal(|ui| {
                ui.label("Scale:");
                if ui.small_button("-").clicked() && ppp > 0.6 {
                    ui.ctx().set_pixels_per_point(ppp - 0.1);
                }
                ui.label(format!("{:.0}%", ppp * 100.0));
                if ui.small_button("+").clicked() && ppp < 2.5 {
                    ui.ctx().set_pixels_per_point(ppp + 0.1);
                }
                if ui.small_button("100%").clicked() {
                    ui.ctx().set_pixels_per_point(1.0);
                }
            });
        });
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Help", |ui| {
            ui.set_min_width(220.0);

            if ui.button("ℹ About crorust.ruxt").clicked() {
                self.state.windows.open_window("About crorust.ruxt");
                ui.close();
            }

            ui.separator();

            ui.hyperlink_to("📖 crorust Documentation", "https://crorust.com");
            ui.hyperlink_to(
                "📦 crorust GitHub Repository",
                "https://github.com/adi1ahmad/crorust",
            );
            ui.hyperlink_to(
                "🐛 Report an Issue",
                "https://github.com/adi1ahmad/crorust/issues",
            );
        });
    }
}

impl eframe::App for RuxtApp {
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        #[cfg(target_os = "android")]
        {
            let events = crate::platform::android::drain_ime_events();
            if !events.is_empty() {
                _raw_input.events.extend(events);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.state.navigation.poll_browser_hash();

        let ctx = ui.ctx().clone();

        #[cfg(not(target_arch = "wasm32"))]
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::B))
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B))
        {
            let sidebar_open = self.state.windows.sidebar_open();
            self.state.windows.set_sidebar_open(!sidebar_open);
        }

        self.state.windows.logic(&ctx);

        self.top_navigation_bar(ui, frame);

        self.state.settings_panel.ui(ui, frame);

        egui::CentralPanel::no_frame().show(ui, |ui| {
            match self.state.navigation.mode {
                crate::navigation::AppMode::Home => {
                    self.website_view.ui(ui, &mut self.state.navigation);
                }
                crate::navigation::AppMode::Workspace => {
                    self.state.windows.ui(ui);
                }
            }
        });

        #[cfg(target_os = "android")]
        {
            let wants_ime = ctx.egui_wants_keyboard_input() || ctx.output(|o| o.ime.is_some());
            crate::platform::android::sync_ime_state(wants_ime);
            if wants_ime {
                ctx.request_repaint();
            }
        }
    }

    #[cfg(feature = "glow")]
    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        self.state.windows.on_exit(gl);
    }
}
