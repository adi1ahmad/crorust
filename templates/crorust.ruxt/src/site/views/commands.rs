use egui::{RichText, Ui};
use super::recipes::RecipeGenerator;

#[derive(Default)]
pub struct CommandsView {
    recipes: RecipeGenerator,
}

impl CommandsView {
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("CLI Commands Reference & Workflow Builder").size(26.0).strong());
        ui.add_space(6.0);
        ui.label(
            RichText::new("Complete reference manual for all crorust CLI commands and parameters.")
                .size(14.0)
                .weak(),
        );
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        self.recipes.ui(ui);

        ui.add_space(24.0);
        ui.heading(RichText::new("Command Encyclopedia").size(20.0).strong());
        ui.add_space(12.0);

        render_cmd_entry(
            ui,
            "crorust i",
            "Initialize & Verify Hermetic Toolchains",
            "Launches the interactive installer or runs headlessly to verify and download minimal LLVM/Clang \
            headers, Android NDK platforms, Java JDK, and Windows/Apple sysroots into ~/.cache/crorust/.\n\
            Flags:\n\
            --non-interactive: Run without curses TUI (ideal for CI/CD runners)\n\
            --check: Verify existing sysroots without downloading",
            "crorust i --non-interactive",
        );

        render_cmd_entry(
            ui,
            "crorust build",
            "Cross-Compile & Package Native Artifacts",
            "Drives Cargo with synthesized flags and invokes in-tree packagers to construct release artifacts.\n\
            Flags:\n\
            --platform <android|apple|windows|linux|web>: Target operating system\n\
            --arch <aarch64|x86_64>: Target CPU architecture\n\
            --apk: Build Android APK\n\
            --aab: Build Android App Bundle\n\
            --dmg: Build macOS DMG installer\n\
            --msix: Build Windows MSIX bundle\n\
            --appimage: Build Linux Type 2 AppImage\n\
            --release: Optimize with LTO and strip debug symbols",
            "crorust build --platform android --apk --release",
        );

        render_cmd_entry(
            ui,
            "crorust run",
            "Deploy, Launch & Stream Hardware Logs",
            "Compiles the binary, deploys it to a physical or emulated device, launches the application, \
            and streams stdout/logcat output back to your workstation terminal.\n\
            Flags:\n\
            --device <device-id>: Specific device identifier (from 'crorust devices')",
            "crorust run --device adb:emulator-5554",
        );

        render_cmd_entry(
            ui,
            "crorust devices",
            "List Connected Hardware & Emulators",
            "Discovers all attached Android devices via ADB, Apple devices/simulators via CoreSimulator bridge, \
            and local desktop display servers.",
            "crorust devices",
        );

        render_cmd_entry(
            ui,
            "crorust cert",
            "Zero-Config Cryptographic Certificates",
            "Generates self-signed developer certificates, PKCS#12 keystores for Android signing, and Authenticode \
            keys for Windows packaging without OpenSSL CLI commands.",
            "crorust cert generate --output debug.p12",
        );

        render_cmd_entry(
            ui,
            "crorust cache",
            "Manage Hermetic Toolchain Cache",
            "Inspects disk usage of ~/.cache/crorust/ (sysroots, Maven AARs, SDK headers) and allows pruning.",
            "crorust cache --stats\ncrorust cache --clean",
        );

        render_cmd_entry(
            ui,
            "crorust template init",
            "Scaffold Multi-Platform Application",
            "Extracts the crorust.ruxt reference boilerplate into a new project directory with ready-to-build \
            desktop, mobile, and web targets.",
            "crorust template init my_awesome_app\ncd my_awesome_app\ncrorust run",
        );
    }
}

fn render_cmd_entry(ui: &mut Ui, syntax: &str, title: &str, desc: &str, example: &str) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(syntax).monospace().strong().size(16.0).color(egui::Color32::from_rgb(31, 111, 235)));
                ui.label(RichText::new(format!("— {}", title)).strong().size(14.0));
            });
            ui.add_space(6.0);
            ui.label(RichText::new(desc).size(13.0).color(ui.visuals().text_color()));
            ui.add_space(8.0);
            crate::rust_view_ui(ui, example);
        });
    ui.add_space(10.0);
}
