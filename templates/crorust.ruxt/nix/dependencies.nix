# =============================================================================
# DEPENDENCY MANAGEMENT - Multi-Host & Modular Package Groups
# =============================================================================
# Partitioned by functional domain and guarded by host OS (`isLinux` / `isDarwin`)
# so that `nix develop` works seamlessly on Linux (x86_64, aarch64) and macOS
# (Apple Silicon aarch64-darwin, Intel x86_64-darwin).
# =============================================================================
{ pkgs, versions }:

let
  inherit (pkgs) lib;
  inherit (pkgs.stdenv) hostPlatform;
  inherit (hostPlatform) isLinux isDarwin;
in
{
  # ===========================================================================
  # 1. CORE DEVELOPMENT - Lightweight essentials for all hosts
  # ===========================================================================
  core = with pkgs; [
    git
    curl
    unzip
    which
    jq
  ];

  # ===========================================================================
  # 2. GRAPHICS, WINDOWING & AUDIO STACK - Host-OS Aware
  # ===========================================================================
  # Note: On Linux, winit/eframe/wgpu dynamically load (`dlopen`) Wayland, X11,
  # libxkbcommon, and Vulkan libraries at runtime. Every library in `graphics`
  # is automatically wired into LD_LIBRARY_PATH and PKG_CONFIG_PATH on Linux.
  graphics =
    lib.optionals isLinux (with pkgs; [
      # --- Direct Rendering & Mesa (Hardware + Lavapipe CPU Software Vulkan) ---
      libdrm
      versions.llvmLib          # Required by Mesa llvmpipe/lavapipe JIT
      mesa
      libGL
      libglvnd

      # --- Vulkan Runtime & Validation ---
      vulkan-loader
      vulkan-headers
      vulkan-validation-layers
      vulkan-extension-layer

      # --- Wayland Display Server ---
      wayland
      wayland-protocols

      # --- X11 Display Server (XWayland & Legacy X11) ---
      xorg.libX11
      xorg.libXi
      xorg.libxcb
      xorg.libXcursor
      xorg.libXrandr
      xorg.libXext
      xorg.libXfixes
      xorg.libxshmfence

      # --- Input, Shaders, Device & Audio ---
      libxkbcommon
      shaderc.lib
      udev
      alsa-lib

      # --- Hardware Video Acceleration (VA-API for media_viewer) ---
      libva
      libpciaccess
    ]
    ++ lib.optionals (isLinux && hostPlatform.isx86_64) (with pkgs; [
      intel-media-driver        # Intel Gen8+ (Broadwell / Skylake / Iris Xe / Arc)
      intel-vaapi-driver        # Legacy Intel (Haswell and older)
    ]))
    ++ lib.optionals isDarwin (with pkgs; [
      libiconv
      shaderc.lib
      apple-sdk
    ] ++ (with pkgs.darwin.apple_sdk.frameworks; [
      AppKit
      Metal
      MetalKit
      Foundation
      QuartzCore
      Security
      SystemConfiguration
      CoreGraphics
      CoreVideo
    ]));

  # ===========================================================================
  # 3. CROSS-TARGET & MOBILE DEPLOYMENT (Aligned with crorust Tier 2 Engine)
  # ===========================================================================
  mobile = with pkgs; [
    # --- Android Standalone Toolchain (No Gradle Bloat!) ---
    # crorust's Tier 2 engine compiles Kotlin/Java directly via kotlinc + javac + d8
    # and packages APKs in pure Rust via crorust-android + crorust-mvn.
    versions.jdk                # Pinned JDK 17 LTS
    kotlin                      # Provides kotlinc for Kotlin bridge compilation
    android-tools               # Provides adb for device deployment & logcat

    # --- iOS Physical Device Deployment ---
    libimobiledevice            # Provides idevice_id, ideviceinfo, idevicedebug
    ideviceinstaller            # Provides ideviceinstaller for iOS .ipa/.app install
  ]
  ++ lib.optionals isLinux (with pkgs; [
    # --- Linux AppImage Acceleration ---
    squashfsTools               # Provides mksquashfs (accelerates crorust-linux)
  ]);

  # ===========================================================================
  # 4. COMPILERS, LINKERS & BUILD TOOLS (LLVM 19 Suite for crorust)
  # ===========================================================================
  # crorust i and CargoBuild drive clang, clang++, llvm-ar, llvm-lib,
  # llvm-readobj, lld, and lldb directly for cross-target compilation.
  build = with pkgs; [
    pkg-config
    stdenv.cc
    stdenv.cc.bintools
    stdenv.cc.cc.lib

    # Pinned LLVM 19 toolchain
    versions.llvmPackages.clang
    versions.llvmPackages.lld
    versions.llvmPackages.lldb
    versions.llvmPackages.llvm  # Provides llvm-ar, llvm-lib, llvm-readobj
  ]
  ++ lib.optionals isLinux (with pkgs; [
    wayland-scanner
  ]);

  # ===========================================================================
  # 5. WEBASSEMBLY TOOLING
  # ===========================================================================
  # Note: Trunk automatically downloads and manages the exact matching
  # wasm-bindgen-cli version corresponding to Cargo.lock at runtime into
  # ~/.cache/trunk. Binaryen provides wasm-opt for size optimization.
  web = with pkgs; [
    trunk                       # WASM web bundler (`trunk serve` / `trunk build`)
    binaryen                    # Provides wasm-opt for release size optimization
  ];

  # ===========================================================================
  # 6. OPT-IN GRAPHICS DEBUGGING TOOLS (`nix develop .#debug` only)
  # ===========================================================================
  # Kept out of the default devShell to save ~350 MB of Qt/Python/Demo closures.
  debugTools =
    lib.optionals isLinux (with pkgs; [
      mesa-demos                # glxinfo, glxgears
      vulkan-tools              # vulkaninfo, vkcube
      libva-utils               # vainfo
      spirv-tools               # spirv-dis, spirv-val
      pciutils                  # lspci
      usbutils                  # lsusb
    ] ++ lib.optional hostPlatform.isx86_64 renderdoc)
    ++ lib.optionals isDarwin (with pkgs; [
      vulkan-tools
      spirv-tools
    ]);
}