use super::frame_history::FrameHistory;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RunMode {

    #[default]
    Reactive,

    Continuous,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct SettingsPanel {
    pub open: bool,

    #[cfg_attr(feature = "serde", serde(skip))]
    run_mode: RunMode,

    #[cfg_attr(feature = "serde", serde(skip))]
    frame_history: FrameHistory,

    egui_windows: EguiWindows,
}

impl SettingsPanel {
    pub fn end_of_top_bar(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let is_compact = crate::is_mobile(ui.ctx()) || ui.available_width() < 420.0;
        let label = if is_compact { "⚙" } else { "⚙ Settings" };
        ui.toggle_value(&mut self.open, label)
            .on_hover_text("Configure display, rendering, and diagnostic preferences");
    }

    pub fn fps(&self) -> f32 {
        self.frame_history.fps()
    }

    pub fn mean_frame_time(&self) -> f32 {
        self.frame_history.mean_frame_time()
    }

    pub fn update(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        self.frame_history
            .on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);

        if self.run_mode == RunMode::Continuous {
            ctx.request_repaint();
        }
    }

    pub fn end_of_frame(&mut self, ctx: &egui::Context) {
        self.egui_windows.windows(ctx);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.update(ui.ctx(), frame);

        let mut open = self.open;
        let mut close = false;

        egui::Panel::left("settings_side_panel")
            .resizable(true)
            .size_range(180.0..=450.0)
            .default_size(240.0)
            .show_collapsible(ui, &mut open, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("⏴")
                            .on_hover_text("Collapse Settings Panel")
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                    self.appearance_ui(ui);
                    ui.separator();

                    self.run_mode_ui(ui);
                    ui.separator();

                    self.frame_history.ui(ui);
                    ui.separator();

                    ui.label("Diagnostic Windows:");
                    self.egui_windows.checkboxes(ui);
                    ui.separator();

                    self.integration_ui(ui, frame);

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        ui.separator();
                        if ui.button("Quit ruxt").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
            });

        if close {
            open = false;
        }
        self.open = open;

        self.end_of_frame(ui.ctx());
    }

    fn appearance_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Display Scaling:");
        let mut pixels_per_point = ui.ctx().pixels_per_point();
        if ui
            .add(egui::Slider::new(&mut pixels_per_point, 0.5..=3.0))
            .changed()
        {
            ui.ctx().set_pixels_per_point(pixels_per_point);
        }
        if ui.button("Reset Scale").clicked() {
            ui.ctx().set_pixels_per_point(1.0);
        }
    }

    fn run_mode_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Run Mode:");
        ui.radio_value(&mut self.run_mode, RunMode::Reactive, "Reactive")
            .on_hover_text("Repaints only upon user interaction or active animations");
        ui.radio_value(&mut self.run_mode, RunMode::Continuous, "Continuous")
            .on_hover_text("Continuous repaints at full display refresh rate");

        if self.run_mode == RunMode::Continuous {
            ui.label(
                egui::RichText::new(format!("Continuous FPS: {:.1}", self.frame_history.fps()))
                    .small()
                    .weak(),
            );
        }
    }

    fn integration_ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if ui.checkbox(&mut fullscreen, "Fullscreen (F11)").changed() {
                ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
            }
            ui.add_space(4.0);
        }

        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(egui::RichText::new("Engine:").small().strong());
                ui.label(egui::RichText::new("crorust · egui").small().weak())
                    .on_hover_text(
                        "Toolchain & Engine: crorust (https://crorust.com) · egui 0.36.2 & eframe",
                    );
            });

            #[cfg(feature = "glow")]
            if _frame.gl().is_some() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(egui::RichText::new("Renderer:").small().strong());
                    ui.label(egui::RichText::new("OpenGL (glow)").small().weak());
                });
            }

            #[cfg(feature = "wgpu")]
            if let Some(render_state) = _frame.wgpu_render_state() {
                let info = render_state.adapter.get_info();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(egui::RichText::new("Renderer:").small().strong());
                    ui.label(
                        egui::RichText::new(format!("wgpu ({:?})", info.backend))
                            .small()
                            .weak(),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(egui::RichText::new("GPU:").small().strong());
                    ui.label(egui::RichText::new(&info.name).small().weak())
                        .on_hover_text(&info.name);
                });
            }
        });
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct EguiWindows {
    settings: bool,
    inspection: bool,
    memory: bool,
    output_events: bool,

    #[cfg_attr(feature = "serde", serde(skip))]
    output_event_history: std::collections::VecDeque<egui::output::OutputEvent>,
}

impl EguiWindows {
    fn checkboxes(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.settings, "🔧 Visual Settings");
        ui.checkbox(&mut self.inspection, "🔍 Inspection");
        ui.checkbox(&mut self.memory, "📝 Memory Stats");
        ui.checkbox(&mut self.output_events, "📤 Output Events");
    }

    fn windows(&mut self, ctx: &egui::Context) {
        ctx.output(|o| {
            for event in &o.events {
                self.output_event_history.push_back(event.clone());
            }
        });
        while self.output_event_history.len() > 1000 {
            self.output_event_history.pop_front();
        }

        egui::Window::new("🔧 Visual Settings")
            .open(&mut self.settings)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.settings_ui(ui);
            });

        egui::Window::new("🔍 Inspection")
            .open(&mut self.inspection)
            .vscroll(true)
            .show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });

        egui::Window::new("📝 Memory Stats")
            .open(&mut self.memory)
            .resizable(false)
            .show(ctx, |ui| {
                ctx.memory_ui(ui);
            });

        egui::Window::new("📤 Output Events")
            .open(&mut self.output_events)
            .resizable(true)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.label("Recent immediate-mode output events emitted by crorust.ruxt:");
                ui.separator();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for event in &self.output_event_history {
                            ui.label(format!("{event:?}"));
                        }
                    });
            });
    }
}
