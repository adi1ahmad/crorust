use egui::{Color32, RichText, Ui};

pub struct TerminalSimulator {
    selected_tab: usize,
    copied_notification: Option<(f64, String)>,
}

impl Default for TerminalSimulator {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            copied_notification: None,
        }
    }
}

impl TerminalSimulator {
    pub fn ui(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(13, 17, 23))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
            .corner_radius(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Top window bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("●").color(Color32::from_rgb(255, 95, 86)).size(12.0));
                    ui.label(RichText::new("●").color(Color32::from_rgb(255, 189, 46)).size(12.0));
                    ui.label(RichText::new("●").color(Color32::from_rgb(39, 201, 63)).size(12.0));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("quickstart — bash")
                            .monospace()
                            .size(11.5)
                            .color(Color32::from_rgb(139, 148, 158)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tabs = ["1. Install", "2. Scaffold App", "3. Cross-Build", "4. Run"];
                        for (idx, tab) in tabs.iter().enumerate().rev() {
                            let selected = self.selected_tab == idx;
                            if ui.selectable_label(selected, RichText::new(*tab).small()).clicked() {
                                self.selected_tab = idx;
                            }
                        }
                    });
                });

                ui.separator();
                ui.add_space(6.0);

                let (command, explanation) = match self.selected_tab {
                    0 => (
                        "# Install crorust CLI and hermetic cross-toolchains\ncargo install crorust\ncrorust i",
                        "crorust i verifies and provisions minimal hermetic LLVM, Android NDK, and Apple SDK headers.",
                    ),
                    1 => (
                        "# Initialize a new multi-platform project from reference template\ncrorust template init my_app\ncd my_app",
                        "Extracts crorust.ruxt boilerplate with ready-to-run desktop, mobile, and web targets.",
                    ),
                    2 => (
                        "# Cross-compile production packages without external SDKs\ncrorust build --platform android --apk\ncrorust build --platform windows --msix\ncrorust build --platform macos --dmg",
                        "Pure-Rust format bundlers package and sign artifacts natively on any development host.",
                    ),
                    _ => (
                        "# Run directly on attached hardware, emulator, or desktop window\ncrorust run --device adb:emulator-5554",
                        "Deploys, launches, and streams live diagnostic logs from the device back to your terminal.",
                    ),
                };

                egui::ScrollArea::horizontal()
                    .id_salt("terminal_snippet_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.monospace(RichText::new(command).color(Color32::from_rgb(201, 209, 217)).size(13.0));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if ui.button("📋 Copy").on_hover_text("Copy snippet to clipboard").clicked() {
                                    ui.ctx().copy_text(command.to_owned());
                                    self.copied_notification = Some((ui.input(|i| i.time), "Copied to clipboard!".to_owned()));
                                }
                            });
                        });
                    });

                ui.add_space(8.0);
                ui.label(RichText::new(explanation).size(12.0).color(Color32::from_rgb(139, 148, 158)));

                if let Some((time, ref text)) = self.copied_notification {
                    if ui.input(|i| i.time) - time < 2.5 {
                        ui.label(RichText::new(text).color(Color32::from_rgb(63, 185, 80)).small());
                    } else {
                        self.copied_notification = None;
                    }
                }
            });
    }
}
