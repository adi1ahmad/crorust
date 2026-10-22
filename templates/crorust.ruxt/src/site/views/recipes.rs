use egui::{RichText, Ui};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostOs {
    Linux,
    MacOs,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetPlatform {
    AndroidApk,
    AndroidAab,
    MacOsDmg,
    IosApp,
    WindowsMsix,
    LinuxAppImage,
    WebAssembly,
}

pub struct RecipeGenerator {
    pub host: HostOs,
    pub target: TargetPlatform,
}

impl Default for RecipeGenerator {
    fn default() -> Self {
        Self {
            host: HostOs::Linux,
            target: TargetPlatform::AndroidApk,
        }
    }
}

impl RecipeGenerator {
    pub fn ui(&mut self, ui: &mut Ui) {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().window_fill)
            .stroke(ui.visuals().window_stroke)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.heading(RichText::new("⚡ Interactive Workflow Recipe Generator").size(18.0).strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Select your development workstation and target platform to generate exact terminal commands.")
                        .size(13.0)
                        .weak(),
                );
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("1. Your Host Workstation:").strong());
                    ui.selectable_value(&mut self.host, HostOs::Linux, "🐧 Linux");
                    ui.selectable_value(&mut self.host, HostOs::MacOs, "🍎 macOS");
                    ui.selectable_value(&mut self.host, HostOs::Windows, "💻 Windows");
                });

                ui.add_space(8.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("2. Target Output:").strong());
                    ui.selectable_value(&mut self.target, TargetPlatform::AndroidApk, "📱 Android (.apk)");
                    ui.selectable_value(&mut self.target, TargetPlatform::AndroidAab, "📱 Android Play Store (.aab)");
                    ui.selectable_value(&mut self.target, TargetPlatform::MacOsDmg, "🍎 macOS (.dmg)");
                    ui.selectable_value(&mut self.target, TargetPlatform::IosApp, "📱 iOS (.app)");
                    ui.selectable_value(&mut self.target, TargetPlatform::WindowsMsix, "💻 Windows (.msix)");
                    ui.selectable_value(&mut self.target, TargetPlatform::LinuxAppImage, "🐧 Linux (.AppImage)");
                    ui.selectable_value(&mut self.target, TargetPlatform::WebAssembly, "🌐 WebAssembly");
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                let recipe = self.synthesize_recipe();

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Synthesized Command Workflow:").strong().size(14.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("📋 Copy Workflow").clicked() {
                            ui.ctx().copy_text(recipe.clone());
                        }
                    });
                });

                ui.add_space(6.0);
                crate::rust_view_ui(ui, &recipe);
            });
    }

    fn synthesize_recipe(&self) -> String {
        let mut steps = Vec::new();

        steps.push("# 1. Provision hermetic cross-toolchains & sysroots\ncrorust i");

        match self.target {
            TargetPlatform::AndroidApk => {
                steps.push("# 2. Cross-compile and sign Android APK (v2 Merkle signature)\ncrorust build --platform android --apk --release");
                steps.push("# 3. Discover connected ADB devices\ncrorust devices");
                steps.push("# 4. Deploy and launch on attached device\ncrorust run --device adb:<device-id>");
            }
            TargetPlatform::AndroidAab => {
                steps.push("# 2. Package Android App Bundle (.aab) for Google Play Store upload\ncrorust build --platform android --aab --release");
            }
            TargetPlatform::MacOsDmg => {
                steps.push("# 2. Cross-compile macOS .app and package read-only .dmg\ncrorust build --platform macos --dmg --release");
            }
            TargetPlatform::IosApp => {
                steps.push("# 2. Cross-compile iOS bundle with ad-hoc signature\ncrorust build --platform ios --release");
            }
            TargetPlatform::WindowsMsix => {
                steps.push("# 2. Package and sign Windows MSIX package\ncrorust build --platform windows --msix --release");
            }
            TargetPlatform::LinuxAppImage => {
                steps.push("# 2. Compile standalone Type 2 SquashFS AppImage\ncrorust build --platform linux --appimage --release");
            }
            TargetPlatform::WebAssembly => {
                steps.push("# 2. Build optimized WebAssembly bundle\ntrunk build --release");
            }
        }

        steps.join("\n\n")
    }
}
