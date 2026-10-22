use crorust::download::env::{is_nix_store_path, LinuxDistro};

#[test]
fn test_is_nix_store_path_detection() {
    assert!(is_nix_store_path("/nix/store/abc12345-jdk-17/bin/java"));
    assert!(is_nix_store_path(
        "/nix/store/xyz98765-gradle-8.7/bin/gradle"
    ));
    assert!(!is_nix_store_path("/usr/bin/java"));
    assert!(!is_nix_store_path(
        "/home/user/.cache/crorust/tools/jdk/bin/java"
    ));
}

#[test]
fn test_linux_distro_execution_capability() {
    let standard = LinuxDistro::StandardFhs;
    assert!(standard.can_run_precompiled_binaries());
    assert!(!standard.is_nixos());

    let nixos_with_ld = LinuxDistro::NixOs {
        has_nix_ld: true,
        in_nix_shell: false,
    };
    assert!(nixos_with_ld.can_run_precompiled_binaries());
    assert!(nixos_with_ld.is_nixos());

    let pure_nixos = LinuxDistro::NixOs {
        has_nix_ld: false,
        in_nix_shell: false,
    };
    assert!(!pure_nixos.can_run_precompiled_binaries());
    assert!(pure_nixos.is_nixos());
}

#[test]
fn test_nixos_diagnostic_error_formatting() {
    let dummy_distro = LinuxDistro::NixOs {
        has_nix_ld: false,
        in_nix_shell: false,
    };

    let err = crorust::download::env::check_nixos_tool_execution_for_distro(
        dummy_distro,
        "adb",
        "android-tools",
    )
    .unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("crorust: NixOS Detected"));
    assert!(err_msg.contains("pkgs.android-tools"));
    assert!(err_msg.contains("The tool `adb` is required"));

    let formatted = crorust::download::env::format_nixos_tool_error("adb", "android-tools");
    assert!(formatted.contains("crorust: NixOS Detected"));
    assert!(formatted.contains("programs.nix-ld.enable = true"));

    let capable_distro = LinuxDistro::NixOs {
        has_nix_ld: true,
        in_nix_shell: false,
    };
    assert!(
        crorust::download::env::check_nixos_tool_execution_for_distro(
            capable_distro,
            "adb",
            "android-tools",
        )
        .is_ok()
    );
}
