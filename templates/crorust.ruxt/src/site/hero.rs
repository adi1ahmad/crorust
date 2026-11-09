use crate::navigation::{AppMode, NavigationState};
use egui::{Color32, RichText, Ui, vec2};

pub fn render_hero(ui: &mut Ui, nav: &mut NavigationState) {
    ui.set_width(ui.available_width());

    // Logo & Impact Header
    let is_compact = crate::is_mobile(ui.ctx()) || ui.available_width() < 500.0;
    if is_compact {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(egui::include_image!("../../assets/ruxt-logo.svg"))
                        .max_height(36.0)
                        .tint(ui.visuals().strong_text_color()),
                );
                ui.add_space(8.0);
                ui.heading(
                    RichText::new("crorust & ruxt")
                        .size(26.0)
                        .strong()
                        .color(ui.visuals().strong_text_color()),
                );
            });
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    RichText::new("The Multi-Host Cross-Compilation Platform for Native Rust Apps")
                        .size(14.0)
                        .color(ui.visuals().weak_text_color()),
                )
                .wrap(),
            );
        });
    } else {
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(egui::include_image!("../../assets/ruxt-logo.svg"))
                    .max_height(48.0)
                    .tint(ui.visuals().strong_text_color()),
            );
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.heading(
                    RichText::new("crorust & ruxt")
                        .size(32.0)
                        .strong()
                        .color(ui.visuals().strong_text_color()),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "The Multi-Host Cross-Compilation Platform for Native Rust Apps",
                        )
                        .size(15.0)
                        .color(ui.visuals().weak_text_color()),
                    )
                    .wrap(),
                );
            });
        });
    }

    ui.add_space(12.0);

    // Authority Badge Strip (crates.io, docs.rs, GitHub, License, Targets)
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 4.0);

        if ui
            .link(
                RichText::new("📦 crates.io v0.1.0")
                    .color(Color32::from_rgb(56, 139, 253))
                    .strong(),
            )
            .on_hover_text("Open crorust on crates.io")
            .clicked()
        {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab("https://crates.io/crates/crorust"));
        }
        ui.label(RichText::new("•").weak());
        if ui
            .link(
                RichText::new("📖 docs.rs API Reference")
                    .color(Color32::from_rgb(52, 168, 83))
                    .strong(),
            )
            .on_hover_text("Browse API docs on docs.rs")
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(
                "https://docs.rs/crorust/latest/crorust/",
            ));
        }
        ui.label(RichText::new("•").weak());
        if ui
            .link(
                RichText::new("⭐ GitHub Repository")
                    .color(Color32::from_rgb(220, 220, 220))
                    .strong(),
            )
            .on_hover_text("Inspect source on GitHub")
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(
                "https://github.com/adi1ahmad/crorust",
            ));
        }
        ui.label(RichText::new("•").weak());
        ui.label(RichText::new("⚖ AGPL-3.0 / MIT").weak());
        ui.label(RichText::new("•").weak());
        ui.label(
            RichText::new("🎯 6 Native Targets")
                .color(Color32::from_rgb(170, 100, 240))
                .strong(),
        );
    });

    ui.add_space(14.0);

    // Core Problem & Value Proposition
    ui.label(
        RichText::new(
            "How do you build cross-platform native apps with Rust? Traditionally, you needed a farm of physical hardware \
            and 30GB of proprietary IDEs: Xcode for Apple, Visual Studio for Windows, and Android Studio for APKs. \
            crorust shatters this barrier: Any developer on Linux, macOS, or Windows can compile, package, and cryptographically \
            sign native binaries for Android, iOS, macOS, Windows, Linux, and WebAssembly from a single unified codebase.",
        )
        .size(14.0)
        .color(ui.visuals().text_color()),
    );

    ui.add_space(16.0);

    // Call to Action Buttons
    let is_mobile = ui.available_width() < 720.0;
    if is_mobile {
        ui.vertical(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("🚀 Explore Platform & Recipes")
                            .size(14.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(31, 111, 235))
                    .min_size(vec2(ui.available_width(), 38.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Home);
            }
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("💻 Launch Template Showcase")
                            .size(14.0)
                            .strong(),
                    )
                    .min_size(vec2(ui.available_width(), 38.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Workspace);
            }
            ui.add_space(8.0);
            let install_cmd = "cargo install crorust";
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("📋 {install_cmd}"))
                            .monospace()
                            .size(13.0),
                    )
                    .min_size(vec2(ui.available_width(), 36.0)),
                )
                .on_hover_text("Click to copy install command to clipboard")
                .clicked()
            {
                ui.ctx().copy_text(install_cmd.to_owned());
            }
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 8.0);

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("🚀 Explore Platform & Recipes")
                            .size(14.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(31, 111, 235))
                    .min_size(vec2(210.0, 36.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Home);
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("💻 Launch Template Showcase")
                            .size(14.0)
                            .strong(),
                    )
                    .min_size(vec2(220.0, 36.0)),
                )
                .clicked()
            {
                nav.navigate_to_mode(AppMode::Workspace);
            }

            let install_cmd = "cargo install crorust";
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("📋 {install_cmd}"))
                            .monospace()
                            .size(13.0),
                    )
                    .min_size(vec2(200.0, 36.0)),
                )
                .on_hover_text("Click to copy install command to clipboard")
                .clicked()
            {
                ui.ctx().copy_text(install_cmd.to_owned());
            }
        });
    }

    ui.add_space(14.0);

    // Target Platform Chips with Natural Wrap and Extend Protection
    ui.label(RichText::new("Supported Targets:").small().weak());
    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

        render_chip(
            ui,
            "🤖 Android (.apk / .aab)",
            Color32::from_rgb(46, 125, 50),
        );
        render_chip(
            ui,
            "🍎 macOS (.dmg / .app)",
            Color32::from_rgb(30, 136, 229),
        );
        render_chip(ui, "📱 iOS (.ipa / .app)", Color32::from_rgb(123, 31, 162));
        render_chip(
            ui,
            "🪟 Windows (.msix / .exe)",
            Color32::from_rgb(0, 120, 212),
        );
        render_chip(ui, "🐧 Linux (.AppImage)", Color32::from_rgb(230, 81, 0));
        render_chip(
            ui,
            "🌐 Web (.wasm / canvas)",
            Color32::from_rgb(0, 150, 136),
        );
    });
}

fn render_chip(ui: &mut Ui, label: &str, border_color: Color32) {
    let is_dark = ui.visuals().dark_mode;
    let alpha = if is_dark { 36 } else { 22 };
    let fill = Color32::from_rgba_premultiplied(
        border_color.r(),
        border_color.g(),
        border_color.b(),
        alpha,
    );
    let text_color = if is_dark {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    let btn = egui::Button::new(RichText::new(label).small().strong().color(text_color))
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, border_color))
        .corner_radius(6.0);

    ui.add(btn);
}
