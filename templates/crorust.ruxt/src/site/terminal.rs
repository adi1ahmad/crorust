use egui::{Color32, RichText, ScrollArea, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostOS {
    Linux,
    MacOs,
    Windows,
}

impl HostOS {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linux => "🐧 Linux",
            Self::MacOs => "🍎 macOS",
            Self::Windows => "🪟 Windows",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetPlatform {
    AndroidApk,
    AndroidAab,
    MacOsDmg,
    IosApp,
    WindowsMsix,
    LinuxAppImage,
    WebAssembly,
}

impl TargetPlatform {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AndroidApk => "Android (.apk)",
            Self::AndroidAab => "Android Store (.aab)",
            Self::MacOsDmg => "macOS (.dmg)",
            Self::IosApp => "Apple iOS (.app)",
            Self::WindowsMsix => "Windows (.msix)",
            Self::LinuxAppImage => "Linux (.AppImage)",
            Self::WebAssembly => "WebAssembly (.wasm)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::AndroidApk => "Android APK",
            Self::AndroidAab => "Android AAB",
            Self::MacOsDmg => "macOS DMG",
            Self::IosApp => "iOS App",
            Self::WindowsMsix => "Windows MSIX",
            Self::LinuxAppImage => "Linux AppImage",
            Self::WebAssembly => "WebAssembly",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WorkbenchTab {
    #[default]
    CliTerminal,
    CiWorkflow,
    ManifestToml,
    PackagingSpec,
}

impl WorkbenchTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CliTerminal => "💻 CLI & Live Terminal",
            Self::CiWorkflow => "⚙️ GitHub Actions CI",
            Self::ManifestToml => "📄 manifest.toml",
            Self::PackagingSpec => "📦 Packaging Spec",
        }
    }
}

pub struct TerminalWorkbench {
    pub host: HostOS,
    pub target: TargetPlatform,
    pub active_tab: WorkbenchTab,
    pub show_build_log: bool,
    copied_notification: Option<(f64, String)>,
    cached_ci_key: (HostOS, TargetPlatform),
    cached_ci_yaml: String,
}

impl Default for TerminalWorkbench {
    fn default() -> Self {
        let host = HostOS::Linux;
        let target = TargetPlatform::AndroidApk;
        let mut wb = Self {
            host,
            target,
            active_tab: WorkbenchTab::CliTerminal,
            show_build_log: true,
            copied_notification: None,
            cached_ci_key: (host, target),
            cached_ci_yaml: String::new(),
        };
        wb.cached_ci_yaml = wb.generate_ci_yaml();
        wb
    }
}

// Backward-compatible alias for existing imports
pub type TerminalSimulator = TerminalWorkbench;

impl TerminalWorkbench {
    pub fn ui(&mut self, ui: &mut Ui) {
        let avail_w = ui.available_width();
        let is_mobile = crate::is_mobile(ui.ctx()) || avail_w < 660.0;

        egui::Frame::new()
            .fill(Color32::from_rgb(13, 17, 23))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
            .corner_radius(8.0)
            .inner_margin(if is_mobile { 12.0 } else { 16.0 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Top window bar with traffic lights and studio title
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("●")
                            .color(Color32::from_rgb(255, 95, 86))
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new("●")
                            .color(Color32::from_rgb(255, 189, 46))
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new("●")
                            .color(Color32::from_rgb(39, 201, 63))
                            .size(12.0),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("crorust cross-compilation workbench")
                            .monospace()
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(201, 209, 217)),
                    );

                    if !is_mobile && self.active_tab == WorkbenchTab::CliTerminal {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut self.show_build_log, "Show Build Logs");
                        });
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(10.0);

                // 1. Host Workstation Selector
                ui.label(
                    RichText::new("1. Your Host Development Machine:")
                        .strong()
                        .size(if is_mobile { 13.0 } else { 14.0 })
                        .color(Color32::from_rgb(201, 209, 217)),
                );
                ui.add_space(4.0);

                if is_mobile {
                    // Mobile: Full-width 3-segment touch bar (38px touch target height)
                    let seg_w = ((avail_w - 24.0) / 3.0).floor();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                        for h in [HostOS::Linux, HostOS::MacOs, HostOS::Windows] {
                            let selected = self.host == h;
                            let btn = egui::Button::new(
                                RichText::new(h.name())
                                    .strong()
                                    .size(13.0)
                                    .color(if selected {
                                        Color32::WHITE
                                    } else {
                                        Color32::from_rgb(180, 185, 195)
                                    }),
                            )
                            .min_size(vec2(seg_w, 38.0))
                            .fill(if selected {
                                Color32::from_rgb(31, 111, 235)
                            } else {
                                Color32::from_rgb(22, 27, 34)
                            });
                            if ui.add(btn).clicked() {
                                self.host = h;
                            }
                        }
                    });
                } else {
                    // Desktop: Clean styled segmented pills
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                        for h in [HostOS::Linux, HostOS::MacOs, HostOS::Windows] {
                            let selected = self.host == h;
                            let btn = egui::Button::new(RichText::new(h.name()).strong().color(
                                if selected {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(180, 185, 195)
                                },
                            ))
                            .min_size(vec2(130.0, 32.0))
                            .fill(if selected {
                                Color32::from_rgb(31, 111, 235)
                            } else {
                                Color32::from_rgb(22, 27, 34)
                            });
                            if ui.add(btn).clicked() {
                                self.host = h;
                            }
                        }
                    });
                }

                ui.add_space(12.0);

                // 2. Target Platform Selector
                ui.label(
                    RichText::new("2. Target Distribution Package:")
                        .strong()
                        .size(if is_mobile { 13.0 } else { 14.0 })
                        .color(Color32::from_rgb(201, 209, 217)),
                );
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for t in [
                        TargetPlatform::AndroidApk,
                        TargetPlatform::AndroidAab,
                        TargetPlatform::MacOsDmg,
                        TargetPlatform::IosApp,
                        TargetPlatform::WindowsMsix,
                        TargetPlatform::LinuxAppImage,
                        TargetPlatform::WebAssembly,
                    ] {
                        let selected = self.target == t;
                        let min_w = if is_mobile {
                            ((avail_w - 32.0) / 2.0).floor()
                        } else {
                            136.0
                        };
                        let btn =
                            egui::Button::new(RichText::new(t.name()).size(12.5).strong().color(
                                if selected {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(180, 185, 195)
                                },
                            ))
                            .min_size(vec2(min_w, 36.0))
                            .fill(if selected {
                                Color32::from_rgb(35, 134, 54)
                            } else {
                                Color32::from_rgb(22, 27, 34)
                            });
                        if ui.add(btn).clicked() {
                            self.target = t;
                        }
                    }
                });

                ui.add_space(14.0);

                // 3. Dynamic Cross-Host Insight Bar
                self.render_insight_bar(ui, is_mobile);

                ui.add_space(14.0);

                // 4. Sub-Navigation Tabs with Responsive Scroll & Extend Protection
                crate::ui::responsive_scroll_strip(ui, "workbench_tabs_scroll", |ui| {
                    for tab in [
                        WorkbenchTab::CliTerminal,
                        WorkbenchTab::CiWorkflow,
                        WorkbenchTab::ManifestToml,
                        WorkbenchTab::PackagingSpec,
                    ] {
                        let selected = self.active_tab == tab;
                        let text_color = if selected {
                            Color32::from_rgb(88, 166, 255)
                        } else {
                            Color32::from_rgb(139, 148, 158)
                        };

                        let btn = egui::Button::new(
                            RichText::new(tab.label())
                                .strong()
                                .size(if is_mobile { 12.5 } else { 13.0 })
                                .color(text_color),
                        )
                        .fill(if selected {
                            Color32::from_rgb(22, 27, 34)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if selected {
                            egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61))
                        } else {
                            egui::Stroke::NONE
                        })
                        .corner_radius(6.0)
                        .min_size(vec2(0.0, if is_mobile { 36.0 } else { 30.0 }));

                        if ui.add(btn).clicked() {
                            self.active_tab = tab;
                        }
                    }
                });

                ui.add_space(8.0);

                // 5. Active Tab View
                match self.active_tab {
                    WorkbenchTab::CliTerminal => self.render_terminal_view(ui, is_mobile),
                    WorkbenchTab::CiWorkflow => self.render_ci_view(ui, is_mobile),
                    WorkbenchTab::ManifestToml => self.render_manifest_view(ui, is_mobile),
                    WorkbenchTab::PackagingSpec => self.render_spec_view(ui, is_mobile),
                }

                // Toast Notification for Copy
                if let Some((time, ref text)) = self.copied_notification {
                    if ui.input(|i| i.time) - time < 2.5 {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("✓ {text}"))
                                .color(Color32::from_rgb(63, 185, 80))
                                .small()
                                .strong(),
                        );
                    } else {
                        self.copied_notification = None;
                    }
                }
            });
    }

    fn render_insight_bar(&self, ui: &mut Ui, is_mobile: bool) {
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(16, 21, 28))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
            .inner_margin(if is_mobile { 10.0 } else { 12.0 })
            .corner_radius(6.0)
            .show(ui, |ui| {
                if is_mobile {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Target:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.target_triple())
                                    .monospace()
                                    .strong()
                                    .size(12.0)
                                    .color(Color32::from_rgb(88, 166, 255)),
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Engine:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.packaging_crate())
                                    .strong()
                                    .color(Color32::from_rgb(170, 100, 240))
                                    .size(12.0),
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Prerequisites:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.prerequisites_text())
                                    .strong()
                                    .color(Color32::from_rgb(63, 185, 80))
                                    .size(12.0),
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Output:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.output_artifact_path())
                                    .monospace()
                                    .strong()
                                    .size(12.0)
                                    .color(Color32::from_rgb(220, 170, 50)),
                            );
                        });
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 6.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                            ui.label(RichText::new("Target Triple:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.target_triple())
                                    .monospace()
                                    .strong()
                                    .size(12.0)
                                    .color(Color32::from_rgb(88, 166, 255)),
                            );
                        });

                        ui.label(RichText::new("•").weak());

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                            ui.label(RichText::new("Engine:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.packaging_crate())
                                    .strong()
                                    .color(Color32::from_rgb(170, 100, 240))
                                    .size(12.0),
                            );
                        });

                        ui.label(RichText::new("•").weak());

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                            ui.label(RichText::new("Prerequisites:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.prerequisites_text())
                                    .strong()
                                    .color(Color32::from_rgb(63, 185, 80))
                                    .size(12.0),
                            );
                        });

                        ui.label(RichText::new("•").weak());

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                            ui.label(RichText::new("Output:").weak().size(12.0));
                            ui.label(
                                RichText::new(self.output_artifact_path())
                                    .monospace()
                                    .strong()
                                    .size(12.0)
                                    .color(Color32::from_rgb(220, 170, 50)),
                            );
                        });
                    });
                }
            });
    }

    fn render_terminal_view(&mut self, ui: &mut Ui, is_mobile: bool) {
        let (command, explanation, log_output) = self.generate_command_and_log();

        if is_mobile {
            // Mobile: Monospace command in horizontally scrollable chip, followed by full-width copy button
            crate::ui::responsive_scroll_strip(ui, "workbench_cmd_mobile_scroll", |ui| {
                ui.monospace(
                    RichText::new(format!("$ {command}"))
                        .color(Color32::from_rgb(88, 166, 255))
                        .strong()
                        .size(13.0),
                );
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(explanation)
                    .size(12.0)
                    .color(Color32::from_rgb(139, 148, 158)),
            );

            ui.add_space(6.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("📋 Copy Command").strong().size(13.0))
                        .min_size(vec2(ui.available_width(), 36.0))
                        .fill(Color32::from_rgb(31, 111, 235))
                        .corner_radius(6.0),
                )
                .clicked()
            {
                ui.ctx().copy_text(command.to_owned());
                self.copied_notification = Some((
                    ui.input(|i| i.time),
                    "Command copied to clipboard!".to_owned(),
                ));
            }
        } else {
            // Desktop: Side-by-side command line with right-aligned copy button
            ui.horizontal(|ui| {
                ui.monospace(
                    RichText::new(format!("$ {command}"))
                        .color(Color32::from_rgb(88, 166, 255))
                        .strong()
                        .size(14.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📋 Copy Command").clicked() {
                        ui.ctx().copy_text(command.to_owned());
                        self.copied_notification = Some((
                            ui.input(|i| i.time),
                            "Command copied to clipboard!".to_owned(),
                        ));
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(explanation)
                    .size(12.5)
                    .color(Color32::from_rgb(139, 148, 158)),
            );
        }

        if self.show_build_log {
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(8, 10, 15))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 35, 45)))
                .corner_radius(4.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ScrollArea::horizontal()
                        .id_salt("workbench_terminal_log_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Lock wrap mode to Extend so log lines preserve columnar alignment
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            ui.monospace(
                                RichText::new(log_output)
                                    .size(11.5)
                                    .color(Color32::from_rgb(201, 209, 217)),
                            );
                        });
                });
        }
    }

    fn ensure_ci_cache(&mut self) {
        let key = (self.host, self.target);
        if key != self.cached_ci_key || self.cached_ci_yaml.is_empty() {
            self.cached_ci_key = key;
            self.cached_ci_yaml = self.generate_ci_yaml();
        }
    }

    fn render_ci_view(&mut self, ui: &mut Ui, is_mobile: bool) {
        self.ensure_ci_cache();
        let ci_yaml = &self.cached_ci_yaml;

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Automated GitHub Actions CI/CD Matrix:")
                    .strong()
                    .size(13.5),
            );

            if !is_mobile {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📋 Copy CI Workflow").clicked() {
                        ui.ctx().copy_text(ci_yaml.to_owned());
                        self.copied_notification = Some((
                            ui.input(|i| i.time),
                            "Workflow YAML copied to clipboard!".to_owned(),
                        ));
                    }
                });
            }
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new("Place this file at .github/workflows/crorust.yml to compile and upload artifacts on push.")
                .size(12.0)
                .color(Color32::from_rgb(139, 148, 158)),
        );

        if is_mobile {
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("📋 Copy CI Workflow").strong())
                        .min_size(vec2(ui.available_width(), 34.0))
                        .fill(Color32::from_rgb(31, 111, 235)),
                )
                .clicked()
            {
                ui.ctx().copy_text(ci_yaml.to_owned());
                self.copied_notification = Some((
                    ui.input(|i| i.time),
                    "Workflow YAML copied to clipboard!".to_owned(),
                ));
            }
        }

        ui.add_space(6.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(8, 10, 15))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 35, 45)))
            .corner_radius(4.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("workbench_ci_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.monospace(
                            RichText::new(ci_yaml)
                                .size(11.5)
                                .color(Color32::from_rgb(201, 209, 217)),
                        );
                    });
            });
    }

    fn render_manifest_view(&mut self, ui: &mut Ui, is_mobile: bool) {
        let toml_snippet = self.generate_manifest_toml();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Canonical manifest.toml snippet for ")
                    .strong()
                    .size(13.5),
            );
            ui.label(
                RichText::new(self.target.short_name())
                    .strong()
                    .size(13.5)
                    .color(Color32::from_rgb(56, 139, 253)),
            );
            ui.label(RichText::new(":").strong().size(13.5));

            if !is_mobile {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📋 Copy TOML").clicked() {
                        ui.ctx().copy_text(toml_snippet.to_owned());
                        self.copied_notification = Some((
                            ui.input(|i| i.time),
                            "manifest.toml snippet copied!".to_owned(),
                        ));
                    }
                });
            }
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Add this configuration table to your project's manifest.toml to customize builds.",
            )
            .size(12.0)
            .color(Color32::from_rgb(139, 148, 158)),
        );

        if is_mobile {
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("📋 Copy TOML").strong())
                        .min_size(vec2(ui.available_width(), 34.0))
                        .fill(Color32::from_rgb(31, 111, 235)),
                )
                .clicked()
            {
                ui.ctx().copy_text(toml_snippet.to_owned());
                self.copied_notification = Some((
                    ui.input(|i| i.time),
                    "manifest.toml snippet copied!".to_owned(),
                ));
            }
        }

        ui.add_space(6.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(8, 10, 15))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 35, 45)))
            .corner_radius(4.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("workbench_toml_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.monospace(
                            RichText::new(toml_snippet)
                                .size(11.5)
                                .color(Color32::from_rgb(201, 209, 217)),
                        );
                    });
            });
    }

    fn render_spec_view(&mut self, ui: &mut Ui, is_mobile: bool) {
        let spec_text = self.generate_packaging_spec();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Format & Wire Specification: ")
                    .strong()
                    .size(13.5),
            );
            ui.label(
                RichText::new(self.target.short_name())
                    .strong()
                    .size(13.5)
                    .color(Color32::from_rgb(56, 139, 253)),
            );

            if !is_mobile {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📋 Copy Specification").clicked() {
                        ui.ctx().copy_text(spec_text.to_owned());
                        self.copied_notification =
                            Some((ui.input(|i| i.time), "Specification copied!".to_owned()));
                    }
                });
            }
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Technical packaging invariants enforced by crorust in-tree member crates.",
            )
            .size(12.0)
            .color(Color32::from_rgb(139, 148, 158)),
        );

        if is_mobile {
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("📋 Copy Specification").strong())
                        .min_size(vec2(ui.available_width(), 34.0))
                        .fill(Color32::from_rgb(31, 111, 235)),
                )
                .clicked()
            {
                ui.ctx().copy_text(spec_text.to_owned());
                self.copied_notification =
                    Some((ui.input(|i| i.time), "Specification copied!".to_owned()));
            }
        }

        ui.add_space(6.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(8, 10, 15))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 35, 45)))
            .corner_radius(4.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("workbench_spec_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.monospace(
                            RichText::new(spec_text)
                                .size(11.5)
                                .color(Color32::from_rgb(201, 209, 217)),
                        );
                    });
            });
    }

    pub fn target_triple(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk | TargetPlatform::AndroidAab => "aarch64-linux-android",
            TargetPlatform::MacOsDmg => "aarch64-apple-darwin",
            TargetPlatform::IosApp => "aarch64-apple-ios",
            TargetPlatform::WindowsMsix => "x86_64-pc-windows-msvc",
            TargetPlatform::LinuxAppImage => "x86_64-unknown-linux-gnu",
            TargetPlatform::WebAssembly => "wasm32-unknown-unknown",
        }
    }

    pub fn packaging_crate(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk | TargetPlatform::AndroidAab => {
                "crorust-android (AXML/v2 Sig)"
            }
            TargetPlatform::MacOsDmg | TargetPlatform::IosApp => "crorust-apple (Mach-O codesign)",
            TargetPlatform::WindowsMsix => "crorust-windows (OPC BlockMap)",
            TargetPlatform::LinuxAppImage => "crorust-linux (backhand SquashFS)",
            TargetPlatform::WebAssembly => "Trunk / wasm-bindgen",
        }
    }

    pub fn prerequisites_text(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk | TargetPlatform::AndroidAab => {
                "Zero Gradle / Android Studio (JDK 17 + NDK r26d)"
            }
            TargetPlatform::MacOsDmg => "Zero Mac / Xcode (Pure-Rust codesign + DMG)",
            TargetPlatform::IosApp => "Zero Mac / Xcode (Ad-hoc CMS Mach-O signature)",
            TargetPlatform::WindowsMsix => "Zero Visual Studio (Hermetic Windows SDK headers)",
            TargetPlatform::LinuxAppImage => "Zero mksquashfs (Pure-Rust backhand gzip)",
            TargetPlatform::WebAssembly => "Hermetic Trunk & wasm-opt driver",
        }
    }

    pub fn output_artifact_path(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk => "target/crorust/android/ruxt.apk",
            TargetPlatform::AndroidAab => "target/crorust/android/ruxt.aab",
            TargetPlatform::MacOsDmg => "target/crorust/macos/ruxt.dmg",
            TargetPlatform::IosApp => "target/crorust/ios/ruxt.app",
            TargetPlatform::WindowsMsix => "target/crorust/windows/ruxt.msix",
            TargetPlatform::LinuxAppImage => "target/crorust/linux/ruxt.AppImage",
            TargetPlatform::WebAssembly => "dist/ (ruxt.wasm, index.html)",
        }
    }

    fn generate_command_and_log(&self) -> (&'static str, &'static str, &'static str) {
        match self.target {
            TargetPlatform::AndroidApk => (
                "crorust build -p android --release",
                "Compiles native Rust shared library, generates AXML/ARSC tables, translates Dalvik bytecode with D8, and signs APK via v2 scheme without Gradle.",
                "[crorust] Resolving target: aarch64-linux-android (API 34, min 23)\n\
[crorust] Toolchain cache: .cache/crorust/ (project scope)\n\
[crorust] Verified Adoptium JDK 17 & Android NDK r26d (streaming SHA-256)\n\
[crorust] Resolving dependencies via PubGrub Maven resolver: androidx.core:1.12.0\n\
[crorust] Compiling libruxt.so via clang (RUSTFLAGS synthesized)\n\
[crorust] Synthesizing Android Binary XML (AXML) with 1-to-1 string pool index binding\n\
[crorust] Compiling Dalvik Executable via D8: classes.dex (4-byte aligned)\n\
[crorust] Assembling APK with 16KB page-aligned native libraries (zero-copy mmap)\n\
[crorust] Cryptographically signing APK with APK Signature Scheme v2 (built-in debug identity)\n\
[crorust] Success: target/crorust/android/ruxt-release.apk (4.1 MB) in 11.2s",
            ),
            TargetPlatform::AndroidAab => (
                "crorust build -p android --format aab --release",
                "Constructs Google Play Store Android App Bundle (.aab) with protocol buffers (proto-XML) and compiled base module.",
                "[crorust] Resolving target: aarch64-linux-android (API 34, min 23)\n\
[crorust] Generating protobuf AndroidManifest.xml and ResourceTable.pb\n\
[crorust] Packaging base module (dex, lib/arm64-v8a, res/)\n\
[crorust] Creating ZIP container with Play Store bundle alignment\n\
[crorust] Success: target/crorust/android/ruxt-release.aab (3.8 MB) in 9.8s",
            ),
            TargetPlatform::WindowsMsix => (
                "crorust build -p windows --release",
                "Compiles x86_64 Windows binary, compiles PRI resource tables, builds OPC container, and signs MSIX with PKCS#7.",
                "[crorust] Resolving target: x86_64-pc-windows-msvc\n\
[crorust] Synthesizing MSVC sysroot headers & lld-link environment\n\
[crorust] Compiling ruxt.exe via rust-lld\n\
[crorust] Compiling Package Resource Index: resources.pri (MRM schema)\n\
[crorust] Generating Open Packaging Conventions (OPC) manifest & 64KB block map\n\
[crorust] Applying PKCS#7 digital signature (built-in debug certificate)\n\
[crorust] Success: target/crorust/windows/ruxt.msix (18.4 MB) in 14.8s",
            ),
            TargetPlatform::MacOsDmg => (
                "crorust build -p macos --format dmg --release",
                "Compiles aarch64 Apple Darwin binary, embeds Info.plist and ICNS, signs Mach-O via pure-Rust apple-codesign, and stitches DMG.",
                "[crorust] Resolving target: aarch64-apple-darwin\n\
[crorust] Compiling binary with synthesized Apple Clang sysroot\n\
[crorust] Synthesizing Info.plist and multi-resolution ICNS icon trees\n\
[crorust] Code-signing Mach-O bundle via pure-Rust apple-codesign (no macOS required)\n\
[crorust] Packaging compressed DMG disk image container\n\
[crorust] Success: target/crorust/macos/ruxt.dmg (12.7 MB) in 9.6s",
            ),
            TargetPlatform::IosApp => (
                "crorust build -p ios --release",
                "Cross-compiles iOS application bundle, generates mobile Info.plist, and signs Mach-O via ad-hoc CMS signature.",
                "[crorust] Resolving target: aarch64-apple-ios\n\
[crorust] Synthesizing iOS SDK headers & framework link args\n\
[crorust] Compiling ruxt iOS executable\n\
[crorust] Signing application payload via apple-codesign\n\
[crorust] Success: target/crorust/ios/ruxt.app in 8.1s",
            ),
            TargetPlatform::LinuxAppImage => (
                "crorust build -p linux --release",
                "Assembles standard Linux AppDir and squashes into a Type-2 AppImage using pure-Rust backhand with zero C compiler requirement.",
                "[crorust] Resolving target: x86_64-unknown-linux-gnu\n\
[crorust] Compiling ruxt ELF binary\n\
[crorust] Assembling AppDir structure (AppRun stub, desktop entry, icon.png)\n\
[crorust] Squashing filesystem via pure-Rust backhand (gzip compression)\n\
[crorust] Prepending Type-2 AppImage runtime ELF header\n\
[crorust] Success: target/crorust/linux/ruxt.AppImage (15.2 MB) in 8.3s",
            ),
            TargetPlatform::WebAssembly => (
                "crorust build -p web",
                "Drives Trunk to compile wasm32-unknown-unknown target, optimize with wasm-opt, and generate dist/ with service worker for PWA support.",
                "[crorust] Resolving target: wasm32-unknown-unknown\n\
[crorust] Driving Trunk with wasm-opt (optimization level 2)\n\
[crorust] Staging assets: manifest.webmanifest, icons, service worker\n\
[crorust] Emitting WebAssembly distribution bundle\n\
[crorust] Success: dist/ (ruxt-app.wasm, index.html, sw.js) in 6.4s",
            ),
        }
    }

    fn generate_ci_yaml(&self) -> String {
        format!(
            r#"# .github/workflows/crorust.yml
name: crorust Multi-Platform Release

on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: {}

      - name: Cache crorust Toolchains & Sysroots
        uses: actions/cache@v4
        with:
          path: ~/.cache/crorust
          key: crorust-toolchains-${{{{ runner.os }}}}-${{{{ hashFiles('**/manifest.toml') }}}}
          restore-keys: |
            crorust-toolchains-${{{{ runner.os }}}}-

      - name: Install crorust CLI
        run: cargo install crorust

      - name: Initialize Hermetic Toolchains
        run: crorust i --non-interactive

      - name: Build {} Package
        run: {}

      - name: Upload Build Artifact
        uses: actions/upload-artifact@v4
        with:
          name: {}-package
          path: {}"#,
            self.target_triple(),
            self.target.short_name(),
            self.command_line(),
            self.target.short_name().to_lowercase().replace(' ', "-"),
            self.output_artifact_path()
        )
    }

    fn command_line(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk => "crorust build -p android --apk --release",
            TargetPlatform::AndroidAab => "crorust build -p android --format aab --release",
            TargetPlatform::MacOsDmg => "crorust build -p macos --format dmg --release",
            TargetPlatform::IosApp => "crorust build -p ios --release",
            TargetPlatform::WindowsMsix => "crorust build -p windows --msix --release",
            TargetPlatform::LinuxAppImage => "crorust build -p linux --release",
            TargetPlatform::WebAssembly => "crorust build -p web",
        }
    }

    fn generate_manifest_toml(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk | TargetPlatform::AndroidAab => {
                r#"[android]
package_name = "com.example.myapp"
app_name = "My Cross-Platform App"
version_code = 1
version_name = "0.1.0"
min_sdk_version = 23
target_sdk_version = 34
icon = "assets/icon_512.png"
use_gradle = false # Pure-Rust standalone packaging engine

# Permissions requested in synthesized AndroidManifest.xml:
# permissions = ["android.permission.INTERNET", "android.permission.ACCESS_NETWORK_STATE"]"#
            }
            TargetPlatform::WindowsMsix => {
                r#"[windows]
package_identity = "MyCompany.MyApp"
publisher = "CN=Developer"
publisher_display_name = "My Company"
app_name = "My Cross-Platform App"
version = "0.1.0.0"
icon = "assets/icon_512.png"
# capabilities = ["internetClient", "runFullTrust"]"#
            }
            TargetPlatform::MacOsDmg => {
                r#"[macos]
bundle_identifier = "com.example.myapp"
bundle_name = "My Cross-Platform App"
version = "0.1.0"
icon = "assets/icon_512.png"
# minimum_system_version = "11.0""#
            }
            TargetPlatform::IosApp => {
                r#"[ios]
bundle_identifier = "com.example.myapp"
bundle_name = "My Cross-Platform App"
version = "0.1.0"
icon = "assets/icon_512.png""#
            }
            TargetPlatform::LinuxAppImage => {
                r#"[linux]
app_name = "My Cross-Platform App"
binary_name = "myapp"
categories = "Utility;Development;"
icon = "assets/icon_512.png"
squashfs_backend = "auto" # pure-Rust backhand with host mksquashfs acceleration"#
            }
            TargetPlatform::WebAssembly => {
                r#"[web]
app_name = "My Cross-Platform App"
title = "My Cross-Platform App"
base_path = "/"
canvas_id = "ruxt_canvas""#
            }
        }
    }

    fn generate_packaging_spec(&self) -> &'static str {
        match self.target {
            TargetPlatform::AndroidApk => {
                r#"Packaging Specification: Android APK (APK Signature Scheme v2)
• Format: Aligned ZIP archive conforming to Android Runtime (ART) specifications.
• AXML: Android Binary XML synthesized with 1-to-1 string pool index binding (ResTable_ref).
• ARSC: Resource table with TableTypeSpec and TableType chunks (dense offsets).
• Alignment: 16KB (16384 bytes) page alignment for uncompressed .so libraries (Android 15+).
• DEX: 4-byte aligned classes.dex entries for zero-copy memory mapping.
• Cryptography: APK Signature Block 42 (v2 Merkle tree hash across 1MB chunks)."#
            }
            TargetPlatform::AndroidAab => {
                r#"Packaging Specification: Android App Bundle (AAB)
• Format: ZIP archive containing modular protobuf structures.
• Protocol Buffers: XML files and ResourceTable converted to Protocol Buffer format.
• Modules: base/ module with manifest, dex, and compiled res/ directory.
• Delivery: Ready for distribution via Google Play Store dynamic feature delivery."#
            }
            TargetPlatform::WindowsMsix => {
                r#"Packaging Specification: Windows MSIX (Open Packaging Conventions)
• Format: OPC ZIP container conforming to ISO/IEC 29500-2.
• BlockMap: AppxBlockMap.xml with 64KB cryptographic payload block hashes.
• PRI: Package Resource Index serialized via MRM resource schema chunks.
• Signatures: PKCS#7 detached digital signature in AppxSignature.p7x."#
            }
            TargetPlatform::MacOsDmg => {
                r#"Packaging Specification: macOS .app & .dmg
• Format: Signed Darwin application bundle inside HFS+/APFS disk image.
• Code Signing: Mach-O code directory hashes and CMS signatures via apple-codesign.
• Property List: UTF-8 XML / Binary Info.plist with NSPrincipalClass and CFBundleIdentifier.
• Icons: Apple ICNS binary format with 16x16 up to 1024x1024 Retina mipmaps."#
            }
            TargetPlatform::IosApp => {
                r#"Packaging Specification: Apple iOS .app
• Format: Darwin bundle structure with embedded ad-hoc code signature.
• Mach-O: 64-bit ARM Mach-O executable with iOS framework load commands.
• Assets: Assets.car / AppIcon PNG catalog trees."#
            }
            TargetPlatform::LinuxAppImage => {
                r#"Packaging Specification: Linux Type 2 AppImage
• Format: SquashFS filesystem prepended with a Type 2 AppImage ELF runtime stub.
• SquashFS Backend: Pure-Rust backhand (gzip compression) with zero C compiler dependency.
• Structure: AppRun symlink, .desktop entry file, and standard icons."#
            }
            TargetPlatform::WebAssembly => {
                r#"Packaging Specification: WebAssembly (WASM) & PWA
• Format: wasm32-unknown-unknown binary optimized via binaryen wasm-opt.
• Canvas: WebGL / WebGPU canvas initialization via wasm-bindgen and web-sys.
• PWA: Progressive Web App manifest (manifest.webmanifest) and service worker."#
            }
        }
    }
}
