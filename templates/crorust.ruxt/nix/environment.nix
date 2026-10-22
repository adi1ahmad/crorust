# =============================================================================
# ENVIRONMENT CONFIGURATION - Multi-Host & Cross-Target Environment Synthesis
# =============================================================================
# All attributes returned by this file are merged into `pkgs.mkShell` in flake.nix.
# Linux-specific driver paths (Mesa, Vulkan ICDs, VA-API, LD_LIBRARY_PATH) are
# guarded by `isLinux` so macOS (`aarch64-darwin`, `x86_64-darwin`) evaluates
# and runs cleanly without Linux ELF/DRM contamination.
# =============================================================================
{ pkgs, dependencies, versions, androidSdk }:

let
  inherit (pkgs) lib;
  inherit (pkgs.stdenv) hostPlatform;
  inherit (hostPlatform) isLinux;

  # Architecture suffix for Mesa Vulkan ICD JSON manifests
  mesaArch = if hostPlatform.isAarch64 then "aarch64" else "x86_64";

  # Built-in Clang resource directory (provides <stddef.h>, <stdint.h>, <arm_neon.h>)
  clangUnwrapped = versions.llvmPackages.clang-unwrapped;
  clangResourceDir = "${clangUnwrapped.lib}/lib/clang/${versions.llvmMajor}";

  # Graphics & runtime shared libraries & pkg-config search paths (Linux only)
  linuxLibraryPath = lib.makeLibraryPath (dependencies.graphics ++ [ pkgs.stdenv.cc.cc.lib ]);
  linuxPkgConfigPath = lib.makeSearchPath "lib/pkgconfig" dependencies.graphics;

  # ---------------------------------------------------------------------------
  # 1. UNIVERSAL ENVIRONMENT VARIABLES (Linux & macOS Hosts)
  # ---------------------------------------------------------------------------
  universalEnv = {
    # --- Rust & Logging ---
    RUST_BACKTRACE = "1";
    RUST_LOG = "info";

    # --- Prebuilt Shader Compiler (wgpu / naga) ---
    SHADERC_LIB_DIR = "${pkgs.shaderc.lib}/lib";

    # --- Java & Android Standalone Engine (`crorust` Tier 2) ---
    # Points `crorust` directly to Nix's JDK 17 so it never needs to download
    # a duplicate Adoptium JDK tarball into `.cache/crorust/tools`.
    JAVA_HOME = "${versions.jdk.home}";

    # Smart LLVM 19 Bridge (`clang`, `clang++`, `clang-cl`) prepended in shell-hook.sh
    CRORUST_LLVM_BRIDGE_BIN = "${pkgs.crorust-llvm-bridge}/bin";

    # --- WebAssembly (`wasm32-unknown-unknown`) C/C++ Cross-Toolchain ---
    # Uses unwrapped clang-19 with explicit -resource-dir so crates with C code
    # (e.g. zstd, ring, sqlite) compile to WASM without host glibc contamination.
    CC_wasm32_unknown_unknown = "${clangUnwrapped}/bin/clang-${versions.llvmMajor}";
    CXX_wasm32_unknown_unknown = "${clangUnwrapped}/bin/clang++-${versions.llvmMajor}";
    AR_wasm32_unknown_unknown = "${versions.llvmPackages.llvm}/bin/llvm-ar";
    CFLAGS_wasm32_unknown_unknown = "-resource-dir ${clangResourceDir}";
    CXXFLAGS_wasm32_unknown_unknown = "-resource-dir ${clangResourceDir}";
  }
  # --- Android SDK (when available in Nixpkgs for this host) ---
  // lib.optionalAttrs (androidSdk != null) {
    ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
    ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
  };

  # ---------------------------------------------------------------------------
  # 2. LINUX-SPECIFIC GRAPHICS & DISPLAY ENVIRONMENT (NixOS, Ubuntu/FHS, WSL2)
  # ---------------------------------------------------------------------------
  linuxEnv = lib.optionalAttrs isLinux {
    # Expose Wayland, X11, Vulkan, EGL, and ALSA libraries to dlopen() at runtime.
    # Also includes:
    # - `/run/opengl-driver/lib`: NixOS host kernel-matched GPU drivers (NVIDIA/AMD/Intel)
    # - `/usr/lib/wsl/lib`: Windows Subsystem for Linux (WSL2) DirectX 12 (`libdxcore.so`)
    # Note: Preexisting host LD_LIBRARY_PATH is preserved and appended cleanly in shell-hook.sh
    LD_LIBRARY_PATH = "${linuxLibraryPath}:/run/opengl-driver/lib:/usr/lib/wsl/lib";
    PKG_CONFIG_PATH = linuxPkgConfigPath;

    # --- Vulkan Loader & Validation Layers ---
    VULKAN_SDK = "${pkgs.vulkan-headers}";
    VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";

    # Expose all hardware Vulkan ICDs + Lavapipe (CPU software Vulkan for deterministic
    # `CI=1 cargo test` egui_kittest visual snapshots) + NixOS & FHS NVIDIA ICDs.
    VK_ICD_FILENAMES = lib.concatStringsSep ":" ([
      "${pkgs.mesa}/share/vulkan/icd.d/intel_icd.${mesaArch}.json"        # Intel Gen9+ / Iris Xe / Arc (ANV)
      "${pkgs.mesa}/share/vulkan/icd.d/intel_hasvk_icd.${mesaArch}.json"  # Intel Gen7/Gen8 Legacy (HasVK)
      "${pkgs.mesa}/share/vulkan/icd.d/radeon_icd.${mesaArch}.json"       # AMD Radeon (RADV)
      "${pkgs.mesa}/share/vulkan/icd.d/dzn_icd.${mesaArch}.json"          # WSL2 DirectX 12 (Dozen)
      "${pkgs.mesa}/share/vulkan/icd.d/virtio_icd.${mesaArch}.json"       # QEMU/KVM VirtIO-GPU Venus
      "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.${mesaArch}.json"          # Lavapipe (CPU Software Renderer)
      "/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.${mesaArch}.json" # NixOS Host NVIDIA
      "/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json"             # NixOS Host NVIDIA (arch-agnostic)
      "/usr/share/vulkan/icd.d/nvidia_icd.json"                           # Standard FHS Linux NVIDIA
      "/etc/vulkan/icd.d/nvidia_icd.json"                                 # Fallback FHS NVIDIA
    ] ++ lib.optionals hostPlatform.isAarch64 [
      "${pkgs.mesa}/share/vulkan/icd.d/panfrost_icd.aarch64.json"         # ARM Mali (Panfrost)
      "${pkgs.mesa}/share/vulkan/icd.d/freedreno_icd.aarch64.json"        # Qualcomm Adreno (Turnip)
      "${pkgs.mesa}/share/vulkan/icd.d/broadcom_icd.aarch64.json"         # Raspberry Pi VideoCore (V3DV)
    ]);

    # Keep Lavapipe visible during Vulkan device enumeration so `CI=1 cargo test`
    # can select `DeviceType::Cpu` while desktop runs still prefer hardware GPUs.
    MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE = "0";

    # --- OpenGL / Mesa DRI Configuration ---
    LIBGL_DRIVERS_PATH = "${pkgs.mesa}/lib/dri:/run/opengl-driver/lib/dri";
    LIBGL_ALWAYS_INDIRECT = "0";
    LIBGL_ALWAYS_SOFTWARE = "0";
    MESA_GLSL_CACHE_DISABLE = "0";

    # --- VA-API Hardware Video Acceleration ---
    LIBVA_DRIVERS_PATH = lib.concatStringsSep ":" (
      lib.optionals hostPlatform.isx86_64 [
        "${pkgs.intel-media-driver}/lib/dri"  # Modern Intel (iHD)
        "${pkgs.intel-vaapi-driver}/lib/dri"  # Legacy Intel (i965)
      ]
      ++ [
        "${pkgs.mesa}/lib/dri"                # AMD Radeon (radeonsi) & Nouveau
        "/run/opengl-driver/lib/dri"          # NixOS Host VA-API drivers
      ]
    );
  };

in
universalEnv // linuxEnv
