#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct About {}

impl crate::Demo for About {
    fn name(&self) -> &'static str {
        "About crorust.ruxt"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(380.0)
            .default_height(540.0)
            .open(open)
            .resizable([true, true])
            .vscroll(true)
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                use crate::View as _;
                self.ui(ui);
            });
    }
}

impl crate::View for About {
    fn ui(&mut self, ui: &mut egui::Ui) {
        use egui::special_emojis::{OS_APPLE, OS_LINUX, OS_WINDOWS};

        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            ui.add(
                egui::Image::new(egui::include_image!("../../../assets/ruxt-logo.svg"))
                    .max_height(34.0)
                    .tint(ui.visuals().strong_text_color()),
            );
            ui.add_space(4.0);
            ui.heading("crorust.ruxt");
            ui.label(egui::RichText::new("Unified Cross-Platform Application Template").small());
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(format!(
            "crorust.ruxt is the reference GUI application template built with crorust (https://crorust.com). \
            It compiles natively to {OS_APPLE} macOS, {OS_LINUX} Linux, {OS_WINDOWS} Windows, \
            Android, iOS, and WebAssembly with zero external toolchain setup.",
        ));

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("Why crorust?")
                .strong()
                .color(ui.visuals().strong_text_color()),
        );
        ui.label(
            "Traditional native development requires gigabytes of platform-specific toolchains: \
            Xcode, Android Studio, Visual Studio, and Gradle wrappers. \
            crorust replaces this with a single unified binary:",
        );

        ui.add_space(6.0);
        ui.label("• Hermetic SDK Provisioning: Auto-fetches and verifies minimal NDK, JDK, and SDK sysroots.");
        ui.label("• Direct LLVM Driving: Clang, LLD, and Cargo are driven directly via synthesized flags.");
        ui.label("• Pure-Rust Packagers: In-tree crates build and sign .apk, .app, .msix, and .AppImage.");

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(
            egui::RichText::new("Build & Deploy Commands")
                .strong()
                .color(ui.visuals().strong_text_color()),
        );

        crate::rust_view_ui(
            ui,
            r#"# Build native packages without external SDKs
crorust build --platform android
crorust build --platform linux --appimage
crorust build --platform windows --msix
crorust build --platform macos --dmg

# Run directly on connected hardware / simulators
crorust run --device <device-id>"#
                .trim(),
        );

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.heading("Links & Documentation");
        links(ui);
    }
}

fn links(ui: &mut egui::Ui) {
    use egui::special_emojis::GITHUB;

    ui.hyperlink_to(
        format!("{GITHUB} crorust GitHub Repository"),
        "https://github.com/adi1ahmad/crorust",
    );
    ui.hyperlink_to("🌐 crorust Documentation & Guides", "https://crorust.com");
    ui.hyperlink_to(
        "📦 crorust.ruxt Template Source",
        "https://github.com/adi1ahmad/crorust/tree/main/templates/crorust.ruxt",
    );
    ui.hyperlink_to(
        "🐛 Issue Tracker",
        "https://github.com/adi1ahmad/crorust/issues",
    );
}
