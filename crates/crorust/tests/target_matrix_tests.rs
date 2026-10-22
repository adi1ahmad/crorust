use clap::Parser;
use crorust::{
    AppleTargetEnv, LinuxLibcVariant, ProjectBuildArguments, TargetArch, TargetCompilationUnit,
    TargetOpt, TargetPlatform,
};

#[test]
fn test_all_standard_rust_triples() {
    let android_arm64 =
        TargetCompilationUnit::new(TargetPlatform::Android, TargetArch::Arm64, TargetOpt::Debug);
    assert_eq!(
        android_arm64.rust_target_triple().unwrap(),
        "aarch64-linux-android"
    );

    let android_x64 =
        TargetCompilationUnit::new(TargetPlatform::Android, TargetArch::X64, TargetOpt::Release);
    assert_eq!(
        android_x64.rust_target_triple().unwrap(),
        "x86_64-linux-android"
    );

    let ios_arm64_device =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Release)
            .with_apple_env(AppleTargetEnv::Device);
    assert_eq!(
        ios_arm64_device.rust_target_triple().unwrap(),
        "aarch64-apple-ios"
    );

    let ios_arm64_sim =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug)
            .with_apple_env(AppleTargetEnv::Simulator);
    assert_eq!(
        ios_arm64_sim.rust_target_triple().unwrap(),
        "aarch64-apple-ios-sim"
    );

    let ios_x64_sim =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::X64, TargetOpt::Debug);
    assert_eq!(
        ios_x64_sim.rust_target_triple().unwrap(),
        "x86_64-apple-ios"
    );

    let macos_arm64 =
        TargetCompilationUnit::new(TargetPlatform::Macos, TargetArch::Arm64, TargetOpt::Release);
    assert_eq!(
        macos_arm64.rust_target_triple().unwrap(),
        "aarch64-apple-darwin"
    );

    let macos_x64 =
        TargetCompilationUnit::new(TargetPlatform::Macos, TargetArch::X64, TargetOpt::Release);
    assert_eq!(
        macos_x64.rust_target_triple().unwrap(),
        "x86_64-apple-darwin"
    );

    let win_x64 =
        TargetCompilationUnit::new(TargetPlatform::Windows, TargetArch::X64, TargetOpt::Release);
    assert_eq!(
        win_x64.rust_target_triple().unwrap(),
        "x86_64-pc-windows-msvc"
    );

    let win_arm64 = TargetCompilationUnit::new(
        TargetPlatform::Windows,
        TargetArch::Arm64,
        TargetOpt::Release,
    );
    assert_eq!(
        win_arm64.rust_target_triple().unwrap(),
        "aarch64-pc-windows-msvc"
    );

    let linux_x64_gnu =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::X64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Gnu);
    assert_eq!(
        linux_x64_gnu.rust_target_triple().unwrap(),
        "x86_64-unknown-linux-gnu"
    );

    let linux_arm64_gnu =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::Arm64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Gnu);
    assert_eq!(
        linux_arm64_gnu.rust_target_triple().unwrap(),
        "aarch64-unknown-linux-gnu"
    );

    let linux_x64_musl =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::X64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Musl);
    assert_eq!(
        linux_x64_musl.rust_target_triple().unwrap(),
        "x86_64-unknown-linux-musl"
    );

    let linux_arm64_musl =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::Arm64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Musl);
    assert_eq!(
        linux_arm64_musl.rust_target_triple().unwrap(),
        "aarch64-unknown-linux-musl"
    );
}

#[test]
fn test_target_directory_key_uniqueness() {
    let t_ios_dev =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug)
            .with_apple_env(AppleTargetEnv::Device);
    let t_ios_sim =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug)
            .with_apple_env(AppleTargetEnv::Simulator);

    assert_ne!(
        t_ios_dev.to_string(),
        t_ios_sim.to_string(),
        "iOS device and simulator must have distinct target display names"
    );
    assert_eq!(t_ios_sim.to_string(), "ios-arm64-sim-debug");

    let t_linux_gnu =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::X64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Gnu);
    let t_linux_musl =
        TargetCompilationUnit::new(TargetPlatform::Linux, TargetArch::X64, TargetOpt::Release)
            .with_linux_libc(LinuxLibcVariant::Musl);

    assert_ne!(
        t_linux_gnu.to_string(),
        t_linux_musl.to_string(),
        "Linux GNU and Musl targets must have distinct target display names"
    );
    assert_eq!(t_linux_musl.to_string(), "linux-x64-musl-release");
}

#[test]
fn test_cli_parser_simulator_and_musl_flags() {
    let config = crorust::config::ManifestConfig::default();

    let args_sim = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "ios",
        "--arch",
        "arm64",
        "--simulator",
    ])
    .expect("Should parse simulator flag");
    let target_sim = args_sim
        .build_target
        .compile_build_target(&config)
        .expect("Should resolve simulator build target");
    assert_eq!(target_sim.target_platform(), TargetPlatform::Ios);
    assert_eq!(target_sim.target_architectures(), &[TargetArch::Arm64]);
    assert_eq!(
        target_sim.target_apple_env(),
        Some(AppleTargetEnv::Simulator)
    );

    let args_musl = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "linux",
        "--arch",
        "x64",
        "--musl",
    ])
    .expect("Should parse musl flag");
    let target_musl = args_musl
        .build_target
        .compile_build_target(&config)
        .expect("Should resolve musl build target");
    assert_eq!(target_musl.target_platform(), TargetPlatform::Linux);
    assert_eq!(target_musl.target_architectures(), &[TargetArch::X64]);
    assert_eq!(
        target_musl.target_linux_libc(),
        Some(LinuxLibcVariant::Musl)
    );

    let args_ios_x64 =
        ProjectBuildArguments::try_parse_from(["crorust", "--platform", "ios", "--arch", "x64"])
            .expect("Should parse ios x64 args");
    let target_ios_x64 = args_ios_x64
        .build_target
        .compile_build_target(&config)
        .expect("Should resolve ios x64 build target");
    assert_eq!(target_ios_x64.target_platform(), TargetPlatform::Ios);
    assert_eq!(target_ios_x64.target_architectures(), &[TargetArch::X64]);
    assert_eq!(
        target_ios_x64.target_apple_env(),
        Some(AppleTargetEnv::Simulator),
        "iOS x64 must automatically infer Simulator environment without explicit --simulator flag"
    );
    assert_eq!(
        target_ios_x64
            .iter_compile_targets()
            .next()
            .unwrap()
            .rust_target_triple()
            .unwrap(),
        "x86_64-apple-ios"
    );
}

#[test]
fn test_ios_simulator_vs_device_version_flag_selection() {
    let min_ver = "15.0";

    let t_dev =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug)
            .with_apple_env(AppleTargetEnv::Device);
    assert_eq!(
        t_dev.apple_version_min_flag(min_ver).as_deref(),
        Some("-miphoneos-version-min=15.0")
    );
    assert_eq!(t_dev.rust_target_triple().unwrap(), "aarch64-apple-ios");

    let t_sim_arm =
        TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug)
            .with_apple_env(AppleTargetEnv::Simulator);
    assert_eq!(
        t_sim_arm.apple_version_min_flag(min_ver).as_deref(),
        Some("-mios-simulator-version-min=15.0")
    );
    assert_eq!(
        t_sim_arm.rust_target_triple().unwrap(),
        "aarch64-apple-ios-sim"
    );
}

#[test]
fn test_simctl_json_parsing_logic() {
    let raw_json = r#"{
        "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-17-0": [
                {
                    "dataPath": "/Users/dev/Library/Developer/CoreSimulator/Devices/1234/data",
                    "logPath": "/Users/dev/Library/Logs/CoreSimulator/1234",
                    "udid": "A1B2C3D4-E5F6-7890-ABCD-EF1234567890",
                    "isAvailable": true,
                    "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-15-Pro",
                    "state": "Booted",
                    "name": "iPhone 15 Pro"
                },
                {
                    "udid": "00000000-0000-0000-0000-000000000000",
                    "isAvailable": false,
                    "state": "Shutdown",
                    "name": "Unavailable Device"
                }
            ]
        }
    }"#;

    let sims = crorust::devices::AppleSimulator::parse_json(raw_json)
        .expect("Should parse simulator records from JSON");

    assert_eq!(sims.len(), 1);
    let dev = &sims[0];

    assert_eq!(dev.name, "iPhone 15 Pro");
    assert_eq!(dev.state, "Booted");
    assert_eq!(dev.udid, "A1B2C3D4-E5F6-7890-ABCD-EF1234567890");
}
