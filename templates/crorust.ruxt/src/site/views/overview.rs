use egui::{Color32, RichText, Ui};
use crate::navigation::{NavigationState, CrorustChapter};

pub fn render_overview(ui: &mut Ui, nav: &mut NavigationState) {
    ui.heading(RichText::new("crorust: The Multi-Host Cross-Compilation Platform").size(26.0).strong());
    ui.add_space(6.0);
    ui.label(RichText::new("A hermetic build engine and packaging suite for Rust native applications.").size(14.0).weak());
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. The Cross-Compilation Wall").size(18.0));
    ui.label(
        "Historically, compiling software for multiple operating systems forced teams to adopt one of two painful compromises:\n\n\
        • The Siloed Workstation Trap: iOS builds demanded macOS with Xcode; Windows MSIX packaging required Visual Studio; \
          Android required 15GB of Android Studio, Gradle, and Android Gradle Plugin caches. Teams could not reproduce each other's builds.\n\
        • The Cloud CI Bottleneck: Developers pushed blind commits and waited 20 minutes for GitHub Actions runners to report simple signing errors.\n\n\
        crorust breaks down this wall: Any engineer on Linux, macOS, or Windows can compile, package, and cryptographically sign \
        production-ready native applications for all desktop, mobile, and web targets using the same command-line interface on the exact same codebase."
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("2. The Three-Tier Architectural Model").size(18.0));
    ui.label("crorust organizes cross-compilation into three discrete, hermetic tiers:");
    ui.add_space(10.0);

    render_tier_box(
        ui,
        "Tier 1 · Multi-Host Development Workstations",
        "Linux (Ubuntu, Arch, Fedora, NixOS) · macOS (Apple Silicon, Intel) · Windows (x86_64, ARM64)\n\
        Developers write and review code on their preferred OS. Workstation differences (path separators, dylib paths) are normalized.",
        Color32::from_rgb(31, 111, 235),
    );
    ui.add_space(8.0);

    render_tier_box(
        ui,
        "Tier 2 · The crorust Orchestration & Packaging Engine",
        "• Target Resolver: Maps CLI arguments and hardware devices into concrete build targets.\n\
        • Tool Manager: Provisions minimal hermetic sysroots and Clang/LLD headers in ~/.cache/crorust/.\n\
        • Cargo & LLVM Driver: Synthesizes RUSTFLAGS, CFLAGS, sysroots, and linker scripts.\n\
        • Pure-Rust Packagers: Encodes native container formats (.apk, .msix, .dmg, .AppImage) in-tree.",
        Color32::from_rgb(210, 153, 34),
    );
    ui.add_space(8.0);

    render_tier_box(
        ui,
        "Tier 3 · Cross-Platform Deployment Targets",
        "📱 Android: .apk (v2 signed), .aab\n\
        🍎 Apple: macOS signed .app/.dmg, iOS .app/.ipa\n\
        💻 Windows: signed .msix, .exe\n\
        🐧 Linux: Type 2 .AppImage, AppDir\n\
        🌐 Web: WebAssembly + HTML canvas dist/",
        Color32::from_rgb(39, 201, 63),
    );

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        if ui.button("Next: Multi-Host Installation Guide ➔").clicked() {
            nav.navigate_to_crorust(CrorustChapter::Installation);
        }
        if ui.button("Inspect 8 In-Tree Crates ➔").clicked() {
            nav.navigate_to_crorust(CrorustChapter::Crates);
        }
    });
}

fn render_tier_box(ui: &mut Ui, title: &str, body: &str, accent: Color32) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1.5, accent))
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(15.0).color(accent));
            ui.add_space(6.0);
            ui.label(RichText::new(body).size(13.0).color(ui.visuals().text_color()));
        });
}

pub fn render_toolchains(ui: &mut Ui) {
    ui.heading(RichText::new("Toolchains, Sysroots & Hermetic CI").size(24.0).strong());
    ui.add_space(8.0);
    ui.label(
        "crorust maintains absolute hermeticity by managing minimal sysroots in ~/.cache/crorust/sysroots/.\n\n\
        When targeting Android or iOS from Linux, crorust drives LLVM/Clang directly, injecting case-folding \
        virtual filesystems (VFS) to handle Apple SDK case-sensitivity mismatches on Linux."
    );
}

pub fn render_packaging(ui: &mut Ui) {
    ui.heading(RichText::new("Packaging & Cryptographic Signing Internals").size(24.0).strong());
    ui.add_space(8.0);
    ui.label(
        "Packaging in crorust does not invoke external CLI wrappers (like codesign or apksigner).\n\n\
        All signing schemes (APK v2 Merkle trees, Apple CMS blobs, Windows Authenticode PKCS#7) \
        are implemented in pure Rust within crorust-* crates."
    );
}

pub fn render_devices(ui: &mut Ui) {
    ui.heading(RichText::new("Hardware Devices & Debugging").size(24.0).strong());
    ui.add_space(8.0);
    ui.label(
        "crorust devices discovers all connected Android devices via ADB, Apple devices/simulators, \
        and local desktop targets. crorust run --device adb:<id> automatically pushes, launches, \
        and streams logcat output back to your workstation."
    );
}
