#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrateId {
    CrorustCli,
    Android,
    Apple,
    Windows,
    Linux,
    Maven,
    Common,
    RuntimeStub,
}

pub struct CrateInfo {
    pub id: CrateId,
    pub name: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub details: &'static str,
    pub formats: &'static [&'static str],
    pub dependencies: &'static [&'static str],
    pub sample_code: &'static str,
}

pub const ALL_CRATES: &[CrateInfo] = &[
    CrateInfo {
        id: CrateId::CrorustCli,
        name: "crorust",
        title: "CLI Orchestrator & Toolchain Synthesizer",
        summary: "The root command-line tool, configuration parser, and Cargo flag synthesizer.",
        details: "crorust parses manifest.toml, resolves target hardware devices (via ADB and Xcode simulators), \
                 manages the hermetic toolchain cache in ~/.cache/crorust/, and synthesizes RUSTFLAGS, CFLAGS, and linker flags.",
        formats: &["manifest.toml", "Cargo.toml", "SessionBuildTarget"],
        dependencies: &["crorust-android", "crorust-apple", "crorust-windows", "crorust-linux", "crorust-mvn", "crorust-common"],
        sample_code: "crorust build --platform android --apk\ncrorust run --device adb:emulator-5554",
    },
    CrateInfo {
        id: CrateId::Android,
        name: "crorust-android",
        title: "Pure-Rust Android APK & AAB Packaging Engine",
        summary: "Builds and signs Android APK and AAB packages in pure Rust without Gradle or Android Studio.",
        details: "Encodes AndroidManifest.xml into Android Binary XML (AXML) with string pools and resource maps. \
                 Generates binary ARSC resource tables. Implements Android APK Signature Scheme v2 by computing 1MB \
                 Merkle tree chunk hashes and embedding the signing block directly before the ZIP Central Directory.",
        formats: &[".apk", ".aab", "AXML", "resources.arsc", "APK Sig v2 Block"],
        dependencies: &["crorust-common"],
        sample_code: "// Synthesize and sign APK\nlet apk = ApkBuilder::new(manifest)\n    .add_dex(dex_bytes)\n    .add_native_lib(TargetArch::Arm64, lib_bytes)\n    .sign_v2(&keypair)?;",
    },
    CrateInfo {
        id: CrateId::Apple,
        name: "crorust-apple",
        title: "Apple macOS & iOS Packaging Engine (No Xcode)",
        summary: "Packages macOS .app bundles, DMG disk images, and iOS .app/.ipa packages on any host.",
        details: "Parses Mach-O 64-bit binary headers, allocates LC_CODE_SIGNATURE load commands, and constructs \
                 Apple CMS/PKCS#7 signature blobs with SHA-256 CodeDirectory slots. Synthesizes binary Info.plist files \
                 and packages read-only HFS+/APFS DMG images without invoking hdiutil.",
        formats: &[".app", ".dmg", ".ipa", "Info.plist", "Mach-O LC_CODE_SIGNATURE"],
        dependencies: &["crorust-common"],
        sample_code: "// Ad-hoc or developer codesigning of Mach-O binary\nlet signed_macho = apple_engine::sign_macho(&macho_bytes, &code_identity)?;",
    },
    CrateInfo {
        id: CrateId::Windows,
        name: "crorust-windows",
        title: "Windows MSIX & Authenticode Engine",
        summary: "Builds Windows MSIX Open Packaging Conventions (OPC) containers and Authenticode signatures.",
        details: "Generates AppxManifest.xml, Package Resource Index (PRI) resources, and AppxBlockMap.xml \
                 containing 64KB block SHA-256 hashes. Signs the container with X.509 Authenticode certificates.",
        formats: &[".msix", "AppxBlockMap.xml", "AppxManifest.xml", "resources.pri"],
        dependencies: &["crorust-common"],
        sample_code: "// Package and sign MSIX OPC bundle\nlet msix = MsixPackage::builder()\n    .manifest(appx_manifest)\n    .binary(\"app.exe\", exe_bytes)\n    .sign(&cert, &key)?;",
    },
    CrateInfo {
        id: CrateId::Linux,
        name: "crorust-linux",
        title: "Linux AppDir & Type 2 AppImage Engine",
        summary: "Constructs portable Linux AppImages using pure-Rust SquashFS compression.",
        details: "Synthesizes standard Linux AppDir directories (desktop entries, icons, MIME types, dynamic loader paths) \
                 and compresses them into standalone Type 2 AppImages using the backhand SquashFS library.",
        formats: &[".AppImage", "AppDir", "SquashFS", ".desktop"],
        dependencies: &["crorust-common", "backhand"],
        sample_code: "// Build Type 2 AppImage\nlet appimage = AppImageBuilder::new(appdir)\n    .compress_squashfs()\n    .emit_standalone_binary()?;",
    },
    CrateInfo {
        id: CrateId::Maven,
        name: "crorust-mvn",
        title: "Pure-Rust Maven Solver with PubGrub",
        summary: "Hermetic Maven POM dependency solver for Android AAR/JAR dependencies without Gradle.",
        details: "Parses Maven POM XML files, evaluates version ranges, and computes optimal dependency closures \
                 using the PubGrub algorithm. Downloads and verifies SHA-256 hashes into ~/.cache/crorust/mvn/.",
        formats: &["pom.xml", ".aar", ".jar"],
        dependencies: &["pubgrub", "crorust-common"],
        sample_code: "// Solve transitive Android dependencies\nlet solver = MvnSolver::new(repo_url);\nlet resolved_aars = solver.resolve(&[\"androidx.appcompat:appcompat:1.6.1\"])?;",
    },
    CrateInfo {
        id: CrateId::Common,
        name: "crorust-common",
        title: "Shared Foundations, Crypto & Memory Alignment",
        summary: "Shared primitives for cryptographic signing, byte alignment, and ZIP container handling.",
        details: "Provides SHA-256, RSA-2048, and ECDSA P-256 signing operations; 4KB memory page alignment helpers; \
                 and high-performance streaming ZIP readers and writers.",
        formats: &["PKCS#8", "X.509", "ZIP Streams"],
        dependencies: &["ring", "rsa", "x509-parser"],
        sample_code: "// Compute 4KB memory alignment for mmap binary execution\nlet aligned_offset = crorust_common::align_up(current_pos, 4096);",
    },
    CrateInfo {
        id: CrateId::RuntimeStub,
        name: "crorust-runtime-stub",
        title: "Minimal Runtime Linker Stubs",
        summary: "Lightweight runtime stubs satisfying platform C runtime linker requirements.",
        details: "Provides minimal stub entry symbols required when linking Rust binaries against bare-metal or minimal sysroots.",
        formats: &["ELF Stub", "COFF Stub"],
        dependencies: &[],
        sample_code: "// Minimal platform entry stub\n#[no_mangle]\npub extern \"C\" fn _crorust_runtime_init() {}",
    },
];
