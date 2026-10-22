use egui::{RichText, Ui};

const COL_TARGET: f32 = 135.0;
const COL_OUTPUT: f32 = 140.0;
const COL_HOST: f32 = 90.0;
const COL_BUNDLER: f32 = 230.0;
const ROW_HEIGHT: f32 = 22.0;

pub fn render_compatibility_matrix(ui: &mut Ui) {
    ui.label(
        RichText::new("Build for Any Platform: Host vs. Target Matrix")
            .size(22.0)
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new("Build and sign native release packages for any target from Linux, macOS, or Windows.")
            .size(13.0)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(14.0);

    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::horizontal()
                .id_salt("matrix_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("matrix_grid")
                        .striped(true)
                        .spacing([16.0, 12.0])
                        .show(ui, |ui| {
                            ui.add_sized([COL_TARGET, ROW_HEIGHT], egui::Label::new(RichText::new("Target Platform").strong()));
                            ui.add_sized([COL_OUTPUT, ROW_HEIGHT], egui::Label::new(RichText::new("Package Output").strong()));
                            ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new("Linux Host").strong()));
                            ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new("macOS Host").strong()));
                            ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new("Windows Host").strong()));
                            ui.add_sized([COL_BUNDLER, ROW_HEIGHT], egui::Label::new(RichText::new("Pure-Rust Bundler").strong()));
                            ui.end_row();

                            row(ui, "📱 Android", ".apk, .aab", "✅ Native", "✅ Native", "✅ Native", "crorust-android (AXML/v2 Sig)");
                            row(ui, "🍎 macOS", "signed .app, .dmg", "✅ Native", "✅ Native", "✅ Native", "crorust-apple (Mach-O codesign)");
                            row(ui, "📱 Apple iOS", ".app, .ipa", "✅ Native", "✅ Native", "✅ Native", "crorust-apple (Ad-hoc CMS)");
                            row(ui, "💻 Windows", "signed .msix, .exe", "✅ Native", "✅ Native", "✅ Native", "crorust-windows (OPC BlockMap)");
                            row(ui, "🐧 Linux", "Type 2 .AppImage", "✅ Native", "✅ Native", "✅ Native", "crorust-linux (backhand SquashFS)");
                            row(ui, "🌐 WebAssembly", "WASM + HTML dist/", "✅ Native", "✅ Native", "✅ Native", "Trunk / wasm-bindgen");
                        });
                });
        });
}

fn row(ui: &mut Ui, target: &str, output: &str, lin: &str, mac: &str, win: &str, bundler: &str) {
    ui.add_sized([COL_TARGET, ROW_HEIGHT], egui::Label::new(RichText::new(target).strong()));
    ui.add_sized([COL_OUTPUT, ROW_HEIGHT], egui::Label::new(RichText::new(output).monospace()));
    ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new(lin).color(egui::Color32::from_rgb(63, 185, 80))));
    ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new(mac).color(egui::Color32::from_rgb(63, 185, 80))));
    ui.add_sized([COL_HOST, ROW_HEIGHT], egui::Label::new(RichText::new(win).color(egui::Color32::from_rgb(63, 185, 80))));
    ui.add_sized([COL_BUNDLER, ROW_HEIGHT], egui::Label::new(RichText::new(bundler).small().color(ui.visuals().weak_text_color())));
    ui.end_row();
}
