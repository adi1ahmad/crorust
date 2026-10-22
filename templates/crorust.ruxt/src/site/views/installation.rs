use egui::{RichText, Ui};

pub fn render_installation(ui: &mut Ui) {
    ui.heading(RichText::new("Multi-Host Installation Guide").size(26.0).strong());
    ui.add_space(6.0);
    ui.label(
        RichText::new("How to install and verify crorust on Linux, macOS, and Windows.")
            .size(14.0)
            .weak(),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading(RichText::new("1. Universal Installation via Cargo").size(18.0));
    ui.label("If you have Rust and Cargo installed, installing crorust takes two commands on any host:");
    crate::rust_view_ui(
        ui,
        "# 1. Install the CLI binary from crates.io\n\
        cargo install crorust\n\n\
        # 2. Provision hermetic toolchains, sysroots, and compiler headers\n\
        crorust i",
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("2. Linux Installation (Ubuntu, Debian, Fedora, Arch)").size(18.0));
    ui.label("On Linux, ensure LLVM/Clang and standard build libraries are present:");
    crate::rust_view_ui(
        ui,
        "# Ubuntu / Debian:\nsudo apt install build-essential clang lld libssl-dev pkg-config\n\n\
        # Arch Linux:\nsudo pacman -S base-devel clang lld openssl\n\n\
        # Fedora:\nsudo dnf groupinstall \"Development Tools\" && sudo dnf install clang lld openssl-devel",
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("3. NixOS / Hermetic Flake").size(18.0));
    ui.label("The crorust repository includes a reproducible Nix flake:");
    crate::rust_view_ui(
        ui,
        "# Enter hermetic development shell with all sysroots provisioned\n\
        nix develop\n\n\
        # Or build the binary directly\n\
        nix build",
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("4. macOS Installation (Apple Silicon & Intel)").size(18.0));
    ui.label("macOS requires the Xcode Command Line Tools for native host linking:");
    crate::rust_view_ui(
        ui,
        "# Install Xcode command line tools\nxcode-select --install\n\n\
        # Install crorust and initialize sysroots\ncargo install crorust\ncrorust i",
    );

    ui.add_space(16.0);
    ui.heading(RichText::new("5. Windows Installation (PowerShell)").size(18.0));
    ui.label("On Windows 10/11, enable Developer Mode to allow symbolic links and MSIX sideloading:");
    crate::rust_view_ui(
        ui,
        "# Install crorust via Cargo\ncargo install crorust\n\n\
        # Initialize Windows SDK headers and Android tools\ncrorust i",
    );
}
