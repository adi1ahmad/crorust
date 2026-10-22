# croRust High-Level Architecture Overview

## 1. The Core Problem: The Cross-Compilation Wall

In modern software development, compiling software for multiple operating systems usually forces teams to adopt one of two painful compromises:

1. **The Siloed Workstation Trap**: iOS builds must be run on macOS with Xcode; Windows MSIX packaging requires Visual Studio on Windows; Android builds demand 15GB of Android Studio, Gradle, and Android Gradle Plugin (AGP) caches. Team members cannot reproduce each other's builds locally.
2. **The Cloud CI Bottleneck**: Developers write code locally, push blind commits, and wait 20 minutes for GitHub Actions runners to tell them that an iOS codesign identity or Windows manifest XML is malformed.

`crorust` breaks down this wall. Its primary design goal is:
> **Any engineer on Linux, macOS, or Windows can compile, package, and cryptographically sign production-ready native applications for all desktop, mobile, and web targets using the same command-line interface on the exact same codebase.**

---

## 2. The Three-Tier Architectural Model

To make cross-compilation manageable and predictable, `crorust` divides the entire system into three discrete tiers:

```mermaid
flowchart TB
    %% Styling
    classDef host fill:#1f3354,stroke:#388bfd,stroke-width:1.5px,color:#e6edf3
    classDef core fill:#3d2e14,stroke:#d29922,stroke-width:1.5px,color:#e6edf3
    classDef pack fill:#421d22,stroke:#f85149,stroke-width:1.5px,color:#e6edf3
    classDef target fill:#163824,stroke:#3fb950,stroke-width:1.5px,color:#e6edf3

    subgraph T1["Tier 1 · Multi-Host Development Workstations"]
        H_Lin["🐧 Linux Host<br/><i>Ubuntu · Arch · Fedora · NixOS</i>"]
        H_Mac["🍎 macOS Host<br/><i>Apple Silicon (arm64) · Intel (x64)</i>"]
        H_Win["🪟 Windows Host<br/><i>x86_64 · ARM64</i>"]
    end

    subgraph T2["Tier 2 · The crorust Orchestration & Packaging Engine"]
        CLI["⚙️ crorust CLI<br/><i>build · run · deploy · debug · i · devices</i>"]
        Mgr[("📦 Hermetic Tool Manager<br/><i>Tools in tools/<host>/ · Sysroots in sysroots/</i>")]
        Synth["🔧 Cargo & LLVM Driver<br/><i>Synthesizes RUSTFLAGS, CFLAGS, linkers</i>"]
        Pack["📦 Pure-Rust Format Bundlers<br/><i>crorust-android · apple · windows · linux</i>"]

        CLI -->|"Resolve target"| Synth
        Mgr -.->|"Provide sysroots"| Synth
        Synth -->|"Drive cargo build"| Pack
    end

    subgraph T3["Tier 3 · Cross-Platform Deployment Targets"]
        T_And["🤖 Android Mobile<br/><i>.apk (v2 signed) · .aab</i>"]
        T_iOS["📱 Apple iOS<br/><i>.app bundle · .ipa</i>"]
        T_Mac["🍏 Apple macOS<br/><i>signed .app · .dmg</i>"]
        T_Win["🪟 Windows Desktop<br/><i>signed .msix · .exe</i>"]
        T_Lin["🐧 Linux Desktop<br/><i>Type 2 .AppImage · AppDir</i>"]
        T_Web["🌐 WebAssembly<br/><i>WASM + HTML canvas dist/</i>"]
    end

    H_Lin & H_Mac & H_Win ==>|"Same CLI syntax & manifest.toml"| CLI

    Pack -->|".apk"| T_And
    Pack -->|".app"| T_iOS
    Pack -->|".dmg"| T_Mac
    Pack -->|".msix"| T_Win
    Pack -->|".AppImage"| T_Lin
    Pack -->|"dist/"| T_Web

    %% Subgraph Styling (Transparent Backgrounds for Seamless Dark/Light Mode)
    style T1 fill:none,stroke:#388bfd,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style T2 fill:none,stroke:#d29922,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3
    style T3 fill:none,stroke:#3fb950,stroke-width:1.5px,stroke-dasharray:4 4,color:#e6edf3

    class H_Lin,H_Mac,H_Win host
    class CLI,Synth core
    class Mgr,Pack pack
    class T_And,T_iOS,T_Mac,T_Win,T_Lin,T_Web target
```

### Tier 1: Multi-Host Development Workstations
Workstations represent where developers write and review code. Whether running Arch Linux, macOS Sonoma, or Windows 11, the developer experience is uniform. Workstation differences (such as Windows backslashes, macOS dynamic loader paths, or Linux case-sensitive filesystems) are handled internally by the engine so the developer never writes platform-specific wrapper scripts.

### Tier 2: The Orchestration & Packaging Engine
The engine is composed of four cooperating systems:
1. **Target Resolver**: Parses CLI arguments and device IDs (`adb:xyz`, `sim:UUID`, `host`) to produce a concrete `SessionBuildTarget` (platform, architecture, libc variant).
2. **Tool Manager**: Automatically discovers or downloads minimal SDK headers and sysroots into the local `.cache/crorust/` directory.
3. **Environment Synthesizer**: Constructs exact compiler flags (`--sysroot`, `-isystem`, `-Clink-arg=-fuse-ld=lld`) and injects them into Cargo.
4. **Pure-Rust Bundlers**: Receives the raw compiled binary from Cargo and packages it into the platform's native container format without calling proprietary external tools.

### Tier 3: Cross-Platform Deployment Targets
The output deliverables. These are not intermediate files; they are final, production-ready, cryptographically signed binaries ready to be sideloaded onto a physical device or uploaded to the Google Play Store, Apple App Store, or Microsoft Store.

---

## 3. The Lifecycle of a Build

When a developer runs `crorust run --device adb:emulator-5554`, the following sequence executes:

```mermaid
sequenceDiagram
    autonumber
    actor Dev as Developer
    participant CLI as crorust CLI
    participant Device as Device Bridge (ADB)
    participant Tool as Tool Manager
    participant Synth as Environment Synthesizer
    participant Cargo as Cargo / LLVM
    participant Bundler as crorust-android
    participant Target as Android Device

    Dev->>CLI: crorust run --device adb:emulator-5554
    CLI->>Device: Query device architecture & API level
    Device-->>CLI: Architecture: aarch64, API: 34
    CLI->>Tool: Verify NDK sysroot & JDK in .cache/crorust/
    Tool-->>CLI: Sysroot ready at .cache/crorust/tools/.../ndk/
    CLI->>Synth: Synthesize RUSTFLAGS & CFLAGS for aarch64-linux-android
    Synth-->>CLI: CC=clang, sysroot flags, dynamic linker flags
    CLI->>Cargo: cargo build --target aarch64-linux-android
    Cargo-->>CLI: Emits libmy_app.so at target/.../release/
    CLI->>Bundler: Assemble APK (AXML manifest, ARSC resources, page alignment)
    Bundler->>Bundler: Apply APK Signature Scheme v2 (Merkle tree)
    Bundler-->>CLI: Emits my_app.apk
    CLI->>Device: adb install -r my_app.apk
    CLI->>Device: adb shell am start -n com.example.myapp/.MainActivity
    CLI->>Dev: Stream logcat output directly to terminal!
```

---

## 4. Key Architectural Contracts

The codebase relies on four foundational traits defined in [`crates/crorust/src/core/contracts.rs`](../crates/crorust/src/core/contracts.rs):

### 1. `ToolchainProvider`
Defines how platform SDKs and sysroots are located:
- `resolve_android_ndk()`: Finds or downloads the Android NDK sysroot headers and cross-compilers.
- `resolve_apple_sdk(platform)`: Locates `MacOSX.sdk` or `iPhoneOS.sdk`.
- `resolve_windows_sdk()`: Provides CRT headers and UCRT import libraries.

### 2. `BuildEnvironmentSynthesizer`
Generates compiler environment variables:
- Injects `-Clink-arg=-fuse-ld=lld` or `-Clinker=rust-lld`.
- Injects sysroot header search paths so C dependencies (`cc-rs`, `cmake-rs`) find cross-headers.

### 3. `PackageAssembler`
Accepts compiled binaries and manifest configurations, generating the final container format.

---

## 5. Directory Structure & Workspace Layout

```
crorust/
├── Cargo.toml                          # Workspace root manifest
├── README.md                           # Main documentation entrypoint
├── docs/                               # In-depth architectural guides
│   ├── overview.md                     # This document
│   ├── toolchains-and-sysroots.md      # Toolchain download & caching mechanics
│   ├── packaging-internals.md          # Pure-Rust packaging internals
│   ├── devices-and-debugging.md        # ADB, usbmuxd, and LLDB debugging
│   ├── configuration-guide.md          # manifest.toml reference
│   ├── template-guide.md               # Reference app (crorust.ruxt) guide
│   ├── cache-and-ci.md                 # Caching and CI matrix manual
│   └── crates/                         # Crate-specific architectural manuals
├── crates/
│   ├── crorust/                        # Orchestrator CLI & LLVM driver
│   ├── crorust-android/                # AXML, ARSC, APK Sig v2, AAB
│   ├── crorust-apple/                  # macOS/iOS .app, codesign, Notarization
│   ├── crorust-windows/                # MSIX OPC, 64KB block map, PRI, Authenticode
│   ├── crorust-linux/                  # AppDir, SquashFS backhand, Type 2 AppImage
│   ├── crorust-mvn/                    # PubGrub Maven dependency resolution
│   ├── crorust-common/                 # Alignment sinks, ELF parser, Clang VFS
│   └── crorust-runtime-stub/           # Minimal ELF AppImage runtime launcher
└── templates/
    └── crorust.ruxt/                   # Embedded reference application
```
