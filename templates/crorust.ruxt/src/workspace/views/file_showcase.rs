use std::sync::Arc;

use crate::Demo;
use crate::platform::dialog::{self, PickedFile};
use egui::{Color32, RichText, Slider, Ui, vec2};

#[derive(Default)]
pub struct UniversalFileShowcase {
    current_file: Option<Arc<PickedFile>>,
    zoom: f32,
    status_message: Option<String>,
    last_picked_generation: u64,
    current_image_uri: Option<String>,
}

impl Demo for UniversalFileShowcase {
    fn name(&self) -> &'static str {
        "📁 Universal File & Media Showcase"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        // Poll for asynchronous file pick events across desktop, WASM, or mobile
        if let Some(file) = dialog::poll_picked_file_subscriber(&mut self.last_picked_generation) {
            self.status_message = Some(format!(
                "Successfully imported: {} ({:.1} KB)",
                file.name,
                file.bytes.len() as f32 / 1024.0
            ));
            self.zoom = 1.0;
            if file.is_image() {
                if let Some(ref old_uri) = self.current_image_uri {
                    ui.ctx().forget_image(old_uri);
                }
                let uri = format!(
                    "bytes://picked/{}/{}",
                    self.last_picked_generation, file.name
                );
                ui.ctx().include_bytes(uri.clone(), file.bytes.clone());
                self.current_image_uri = Some(uri);
            } else if let Some(ref old_uri) = self.current_image_uri {
                ui.ctx().forget_image(old_uri);
                self.current_image_uri = None;
            }
            self.current_file = Some(file);
        }

        // Poll for native Drag & Drop events
        #[cfg(not(target_arch = "wasm32"))]
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                let name = dropped
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "dropped_file".to_string());
                if let Ok(bytes) = dropped.bytes() {
                    self.last_picked_generation += 1;
                    self.status_message = Some(format!(
                        "Dropped file: {} ({:.1} KB)",
                        name,
                        bytes.len() as f32 / 1024.0
                    ));
                    self.zoom = 1.0;
                    let picked = Arc::new(PickedFile::new(name.clone(), bytes));
                    if picked.is_image() {
                        if let Some(ref old_uri) = self.current_image_uri {
                            ui.ctx().forget_image(old_uri);
                        }
                        let uri =
                            format!("bytes://picked/{}/{}", self.last_picked_generation, name);
                        ui.ctx().include_bytes(uri.clone(), picked.bytes.clone());
                        self.current_image_uri = Some(uri);
                    } else if let Some(ref old_uri) = self.current_image_uri {
                        ui.ctx().forget_image(old_uri);
                        self.current_image_uri = None;
                    }
                    self.current_file = Some(picked);
                }
            }
        });

        let is_mobile = crate::is_mobile(ui.ctx());
        let screen = ui.ctx().content_rect();
        let default_w = (screen.width() * 0.85).clamp(320.0, 720.0);
        let default_h = (screen.height() * 0.75).clamp(280.0, 560.0);

        let mut window = egui::Window::new(self.name())
            .open(open)
            .default_size(vec2(default_w, default_h))
            .max_size(vec2(screen.width() - 16.0, screen.height() - 32.0))
            .constrain_to(screen);

        if is_mobile {
            window = window
                .fixed_pos([screen.min.x + 8.0, screen.min.y + 42.0])
                .fixed_size([screen.width() - 16.0, screen.height() - 54.0]);
        }

        window.show(ui.ctx(), |ui| {
            self.ui(ui);
        });
    }
}

impl UniversalFileShowcase {
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(RichText::new("📂 Open Local File…").strong().color(Color32::WHITE))
                .on_hover_text("Open file picker (supported across Desktop, Android, and WebAssembly)")
                .clicked()
            {
                dialog::pick_file_async();
            }

            ui.separator();
            ui.label(RichText::new("Quick Previews:").weak());

            if ui.small_button("🖼 Sample Image").clicked() {
                self.last_picked_generation += 1;
                let bytes = include_bytes!("../../../assets/icon_512.png").to_vec();
                let picked = Arc::new(PickedFile::new("icon_512.png", bytes));
                if let Some(ref old_uri) = self.current_image_uri {
                    ui.ctx().forget_image(old_uri);
                }
                let uri = format!("bytes://sample/{}/icon_512.png", self.last_picked_generation);
                ui.ctx().include_bytes(uri.clone(), picked.bytes.clone());
                self.current_image_uri = Some(uri);
                self.current_file = Some(picked);
                self.zoom = 1.0;
                self.status_message =
                    Some("Loaded embedded icon_512.png (High-Resolution RGBA)".to_owned());
            }

            if ui.small_button("📄 Sample Markdown").clicked() {
                let md_sample = b"# How to Build Cross-Platform Native Apps with Rust\n\n\
Welcome to **ruxt**, the multi-platform starter template powered by **crorust**!\n\n\
## Key Architectural Features\n\
- **Zero-Config Cross-Compilation:** Compile from Linux, macOS, or Windows.\n\
- **Pure-Rust Format Packagers:** Encodes APK, MSIX, DMG, and AppImage containers directly.\n\
- **Universal File Import:** Memory-backed `PickedFile` working seamlessly in WebAssembly and desktop.\n\n\
### Quick CLI Syntax\n\
```rust\n\
// Cross-compiling for Android without Gradle or Android Studio:\n\
// $ crorust build -p android --release\n\
fn main() {\n\
    println!(\"Hello from native cross-platform Rust!\");\n\
}\n\
```\n\n\
*Links:* [crorust.com](https://crorust.com) | [crates.io/crates/crorust](https://crates.io/crates/crorust)\n"
                    .to_vec();
                self.current_file = Some(Arc::new(PickedFile::new("README.md", md_sample)));
                self.status_message = Some("Loaded Sample Markdown document".to_owned());
            }

            if ui.small_button("🦀 Sample Rust Code").clicked() {
                let rs_sample = b"// Entry point for ruxt cross-platform native app\n\
use eframe::NativeOptions;\n\n\
fn main() -> eframe::Result {\n\
    let options = NativeOptions::default();\n\
    eframe::run_native(\n\
        \"ruxt\",\n\
        options,\n\
        Box::new(|cc| Ok(Box::new(ruxt::RuxtApp::new(cc)))),\n\
    )\n\
}\n"
                .to_vec();
                self.current_file = Some(Arc::new(PickedFile::new("main.rs", rs_sample)));
                self.status_message = Some("Loaded Sample Rust source file".to_owned());
            }
        });

        ui.add_space(4.0);
        ui.separator();

        if let Some(msg) = &self.status_message {
            ui.label(
                RichText::new(msg)
                    .small()
                    .color(Color32::from_rgb(63, 185, 80)),
            );
            ui.add_space(4.0);
        }

        if let Some(file) = &self.current_file {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "File: {} · Size: {:.1} KB",
                        file.name,
                        file.bytes.len() as f32 / 1024.0
                    ))
                    .strong(),
                );

                if file.is_image() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Reset Zoom").clicked() {
                            self.zoom = 1.0;
                        }
                        ui.add(Slider::new(&mut self.zoom, 0.2..=3.0).text("Zoom"));
                    });
                }
            });

            ui.separator();

            if file.is_image() {
                self.render_image_view(self.zoom, ui, file);
            } else if file.is_markdown() || file.as_str().is_some() {
                Self::render_markdown_view(ui, file);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("Binary file loaded. Direct graphical preview unavailable.");
                });
            }
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(
                    RichText::new("📁 Drop any file here or click 'Open Local File…'")
                        .size(16.0)
                        .weak(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Supported: PNG, JPEG, SVG, WebP, Markdown (.md), and Rust source files (.rs).\n\
                        Runs seamlessly in WebAssembly, Android, and Desktop without filesystem paths.",
                    )
                    .small()
                    .weak(),
                );
            });
        }
    }

    fn render_image_view(&self, zoom: f32, ui: &mut Ui, file: &PickedFile) {
        let uri = self
            .current_image_uri
            .clone()
            .unwrap_or_else(|| format!("bytes://{}", file.name));
        egui::ScrollArea::both()
            .id_salt("file_showcase_image_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let zoom_scale = if zoom <= 0.01 { 1.0 } else { zoom };
                ui.add(
                    egui::Image::new(uri)
                        .fit_to_original_size(zoom_scale)
                        .corner_radius(4.0),
                );
            });
    }

    fn render_markdown_view(ui: &mut Ui, file: &PickedFile) {
        egui::ScrollArea::both()
            .id_salt("file_showcase_md_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(text) = file.as_str() {
                    crate::markdown::easy_mark(ui, text);
                } else {
                    ui.label("Failed to decode text as UTF-8.");
                }
            });
    }
}
