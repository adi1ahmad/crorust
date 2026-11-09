use crate::ui::ScreenTier;
use egui::{Color32, RichText, Ui};

const COMP_COL_TARGET: f32 = 120.0;
const COMP_COL_TRAD: f32 = 340.0;
const COMP_COL_CRORUST: f32 = 360.0;

const COL_TARGET: f32 = 135.0;
const COL_OUTPUT: f32 = 150.0;
const COL_HOST: f32 = 95.0;
const COL_BUNDLER: f32 = 250.0;
const ROW_HEIGHT: f32 = 22.0;

const COMP_DATA: [(&str, &str, &str); 6] = [
    (
        "Android",
        "Android Studio, 15GB Gradle caches, aapt2, apksigner",
        "crorust build -p android (D8 + PubGrub + APK v2)",
    ),
    (
        "Apple macOS",
        "Physical Mac, Xcode, Apple Notary tool, create-dmg",
        "crorust build -p macos --format dmg (apple-codesign)",
    ),
    (
        "Apple iOS",
        "Physical Mac, Xcode, Apple Developer certs",
        "crorust build -p ios (ad-hoc CMS signing)",
    ),
    (
        "Windows",
        "Visual Studio, Windows SDK, MakeAppx, signtool",
        "crorust build -p windows (OPC + PRI + PKCS#7)",
    ),
    (
        "Linux",
        "linuxdeploy, system mksquashfs, AppRun stubs",
        "crorust build -p linux (pure-Rust backhand)",
    ),
    (
        "WebAssembly",
        "Node.js, Webpack, emscripten compiler",
        "crorust build -p web (Trunk + PWA)",
    ),
];

const MATRIX_DATA: [(&str, &str, &str, &str, &str, &str); 6] = [
    (
        "📱 Android",
        ".apk, .aab",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "crorust-android (AXML/v2 Sig)",
    ),
    (
        "🍎 macOS",
        "signed .app, .dmg",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "crorust-apple (Mach-O codesign)",
    ),
    (
        "📱 Apple iOS",
        ".app, .ipa",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "crorust-apple (Ad-hoc CMS)",
    ),
    (
        "💻 Windows",
        "signed .msix, .exe",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "crorust-windows (OPC BlockMap)",
    ),
    (
        "🐧 Linux",
        "Type 2 .AppImage",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "crorust-linux (backhand SquashFS)",
    ),
    (
        "🌐 WebAssembly",
        "WASM + HTML dist/",
        "✅ Native",
        "✅ Native",
        "✅ Native",
        "Trunk / wasm-bindgen",
    ),
];

pub fn render_compatibility_matrix(ui: &mut Ui) {
    let is_compact = ScreenTier::from_ui(ui).is_compact();

    ui.add(
        egui::Label::new(
            RichText::new("Build for Any Platform: Host vs. Target Matrix")
                .size(22.0)
                .strong()
                .color(ui.visuals().strong_text_color()),
        )
        .wrap(),
    );
    ui.add_space(6.0);
    ui.add(
        egui::Label::new(
            RichText::new("Build and sign native release packages for any target from Linux, macOS, or Windows — zero external IDEs required.")
                .size(13.0)
                .color(ui.visuals().weak_text_color()),
        )
        .wrap(),
    );
    ui.add_space(14.0);

    // Traditional Overhead vs crorust Comparison
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke())
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("⚡ Traditional Toolchains vs. The crorust Engine")
                    .strong()
                    .size(14.5)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.add_space(8.0);

            if is_compact {
                for (target, trad, crorust) in COMP_DATA {
                    render_comp_card(ui, target, trad, crorust);
                    ui.add_space(6.0);
                }
            } else {
                egui::ScrollArea::horizontal()
                    .id_salt("comp_matrix_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("comparison_grid")
                            .striped(true)
                            .spacing([16.0, 10.0])
                            .show(ui, |ui| {
                                ui.add_sized(
                                    [COMP_COL_TARGET, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Target")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COMP_COL_TRAD, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Traditional Toolchain Required")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COMP_COL_CRORUST, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("crorust Pure-Rust Engine").strong().color(
                                            if ui.visuals().dark_mode {
                                                Color32::from_rgb(56, 139, 253)
                                            } else {
                                                Color32::from_rgb(9, 105, 218)
                                            },
                                        ),
                                    ),
                                );
                                ui.end_row();

                                for (target, trad, crorust) in COMP_DATA {
                                    comp_row(ui, target, trad, crorust);
                                }
                            });
                    });
            }
        });

    ui.add_space(16.0);

    // Host-to-Target Matrix
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("🎯 Multi-Host Workstation Compatibility Matrix")
                    .strong()
                    .size(14.5),
            );
            ui.add_space(8.0);

            if is_compact {
                for (target, output, lin, mac, win, bundler) in MATRIX_DATA {
                    render_matrix_card(ui, target, output, lin, mac, win, bundler);
                    ui.add_space(6.0);
                }
            } else {
                egui::ScrollArea::horizontal()
                    .id_salt("matrix_table_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("matrix_grid")
                            .striped(true)
                            .spacing([16.0, 12.0])
                            .show(ui, |ui| {
                                ui.add_sized(
                                    [COL_TARGET, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Target Platform")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COL_OUTPUT, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Package Output")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COL_HOST, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Linux Host")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COL_HOST, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("macOS Host")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COL_HOST, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Windows Host")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.add_sized(
                                    [COL_BUNDLER, ROW_HEIGHT],
                                    egui::Label::new(
                                        RichText::new("Pure-Rust Bundler")
                                            .strong()
                                            .color(ui.visuals().strong_text_color()),
                                    ),
                                );
                                ui.end_row();

                                for (target, output, lin, mac, win, bundler) in MATRIX_DATA {
                                    row(ui, target, output, lin, mac, win, bundler);
                                }
                            });
                    });
            }
        });
}

fn render_comp_card(ui: &mut Ui, target: &str, trad: &str, crorust: &str) {
    let is_dark = ui.visuals().dark_mode;
    let trad_color = if is_dark {
        Color32::from_rgb(220, 100, 100)
    } else {
        Color32::from_rgb(180, 40, 40)
    };
    let trad_desc_color = if is_dark {
        Color32::from_rgb(180, 185, 195)
    } else {
        ui.visuals().text_color()
    };
    let crorust_color = if is_dark {
        Color32::from_rgb(63, 185, 80)
    } else {
        Color32::from_rgb(46, 125, 50)
    };

    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().window_stroke())
        .corner_radius(6.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(target)
                    .strong()
                    .size(14.0)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Traditional: ").small().color(trad_color));
                ui.label(RichText::new(trad).small().color(trad_desc_color));
            });
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("crorust: ")
                        .small()
                        .strong()
                        .color(crorust_color),
                );
                ui.label(RichText::new(crorust).small().strong().color(crorust_color));
            });
        });
}

fn render_matrix_card(
    ui: &mut Ui,
    target: &str,
    output: &str,
    lin: &str,
    mac: &str,
    win: &str,
    bundler: &str,
) {
    let check_color = if ui.visuals().dark_mode {
        Color32::from_rgb(63, 185, 80)
    } else {
        Color32::from_rgb(46, 125, 50)
    };

    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().window_stroke())
        .corner_radius(6.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(target)
                        .strong()
                        .size(14.0)
                        .color(ui.visuals().strong_text_color()),
                );
                ui.label(
                    RichText::new(output)
                        .monospace()
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Linux:")
                        .small()
                        .color(ui.visuals().text_color()),
                );
                ui.label(RichText::new(lin).small().color(check_color));
                ui.label(
                    RichText::new("|")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(
                    RichText::new("macOS:")
                        .small()
                        .color(ui.visuals().text_color()),
                );
                ui.label(RichText::new(mac).small().color(check_color));
                ui.label(
                    RichText::new("|")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(
                    RichText::new("Windows:")
                        .small()
                        .color(ui.visuals().text_color()),
                );
                ui.label(RichText::new(win).small().color(check_color));
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(bundler)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
}

fn comp_row(ui: &mut Ui, target: &str, trad: &str, crorust: &str) {
    let is_dark = ui.visuals().dark_mode;
    let trad_color = if is_dark {
        Color32::from_rgb(180, 185, 195)
    } else {
        ui.visuals().text_color()
    };
    let crorust_color = if is_dark {
        Color32::from_rgb(63, 185, 80)
    } else {
        Color32::from_rgb(46, 125, 50)
    };

    ui.add_sized(
        [COMP_COL_TARGET, ROW_HEIGHT],
        egui::Label::new(
            RichText::new(target)
                .strong()
                .color(ui.visuals().strong_text_color()),
        ),
    );
    ui.add_sized(
        [COMP_COL_TRAD, ROW_HEIGHT],
        egui::Label::new(RichText::new(trad).color(trad_color)),
    );
    ui.add_sized(
        [COMP_COL_CRORUST, ROW_HEIGHT],
        egui::Label::new(RichText::new(crorust).color(crorust_color).strong()),
    );
    ui.end_row();
}

fn row(ui: &mut Ui, target: &str, output: &str, lin: &str, mac: &str, win: &str, bundler: &str) {
    let check_color = if ui.visuals().dark_mode {
        Color32::from_rgb(63, 185, 80)
    } else {
        Color32::from_rgb(46, 125, 50)
    };

    ui.add_sized(
        [COL_TARGET, ROW_HEIGHT],
        egui::Label::new(
            RichText::new(target)
                .strong()
                .color(ui.visuals().strong_text_color()),
        ),
    );
    ui.add_sized(
        [COL_OUTPUT, ROW_HEIGHT],
        egui::Label::new(
            RichText::new(output)
                .monospace()
                .color(ui.visuals().text_color()),
        ),
    );
    ui.add_sized(
        [COL_HOST, ROW_HEIGHT],
        egui::Label::new(RichText::new(lin).color(check_color)),
    );
    ui.add_sized(
        [COL_HOST, ROW_HEIGHT],
        egui::Label::new(RichText::new(mac).color(check_color)),
    );
    ui.add_sized(
        [COL_HOST, ROW_HEIGHT],
        egui::Label::new(RichText::new(win).color(check_color)),
    );
    ui.add_sized(
        [COL_BUNDLER, ROW_HEIGHT],
        egui::Label::new(
            RichText::new(bundler)
                .small()
                .color(ui.visuals().weak_text_color()),
        ),
    );
    ui.end_row();
}
