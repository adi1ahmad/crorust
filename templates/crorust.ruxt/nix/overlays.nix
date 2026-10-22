# =============================================================================
# CUSTOM OVERLAYS - Multi-Host LLVM Bridge & Graphics Diagnostics Utility
# =============================================================================
final: prev:
let
  inherit (final) lib stdenv;
  inherit (stdenv.hostPlatform) isLinux;
  versions = import ./versions.nix { pkgs = final; };
  llvmPackages = versions.llvmPackages;
  clangUnwrapped = llvmPackages.clang-unwrapped;
  clangResourceDir = "${clangUnwrapped.lib}/lib/clang/${versions.llvmMajor}";
  hostTriple = stdenv.hostPlatform.config;
in
{
  # ---------------------------------------------------------------------------
  # 1. Smart Host/Cross LLVM Bridge (`crorust-llvm-bridge`)
  # ---------------------------------------------------------------------------
  # Solves the classic Nix cross-compilation dilemma:
  # - When invoked for the HOST (no cross `--target` or `--sysroot`), delegates
  #   to Nix's `llvmPackages.clang` cc-wrapper so host glibc/Darwin headers
  #   are found.
  # - When invoked for a CROSS target (e.g., crorust passing `--target=aarch64-linux-android21`
  #   or `--sysroot=...`, or cargo building `wasm32-unknown-unknown`), delegates
  #   directly to `clang-unwrapped` with `-resource-dir` pointing to Nix's split
  #   clang lib output. This prevents Nix's cc-wrapper from injecting host glibc
  #   headers into Android/WASM/iOS/Windows builds while ensuring compiler
  #   intrinsics (<stddef.h>, <stdint.h>, <arm_neon.h>) are always resolved.
  # ---------------------------------------------------------------------------
  crorust-llvm-bridge = final.runCommand "crorust-llvm-bridge-${versions.llvmMajor}" { } ''
    mkdir -p $out/bin

    cat > $out/bin/clang << 'EOF'
    #!/usr/bin/env bash
    is_cross=0
    target_val=""
    skip_next=0

    # Inspect command line flags without modifying or shifting "$@"
    for arg in "$@"; do
      if [[ "$skip_next" -eq 1 ]]; then
        skip_next=0
        if [[ "$arg" != "${hostTriple}"* ]]; then
          is_cross=1
          break
        fi
        continue
      fi
      case "$arg" in
        --sysroot=*|--sysroot)
          is_cross=1
          break
          ;;
        --target=*)
          target_val="''${arg#--target=}"
          if [[ "$target_val" != "${hostTriple}"* ]]; then
            is_cross=1
            break
          fi
          ;;
        --target|-target)
          skip_next=1
          ;;
      esac
    done

    if [[ "$is_cross" -eq 1 ]]; then
      exec "${clangUnwrapped}/bin/clang" -resource-dir "${clangResourceDir}" "$@"
    else
      exec "${llvmPackages.clang}/bin/clang" "$@"
    fi
    EOF
    chmod +x $out/bin/clang

    cat > $out/bin/clang++ << 'EOF'
    #!/usr/bin/env bash
    is_cross=0
    target_val=""
    skip_next=0

    for arg in "$@"; do
      if [[ "$skip_next" -eq 1 ]]; then
        skip_next=0
        if [[ "$arg" != "${hostTriple}"* ]]; then
          is_cross=1
          break
        fi
        continue
      fi
      case "$arg" in
        --sysroot=*|--sysroot)
          is_cross=1
          break
          ;;
        --target=*)
          target_val="''${arg#--target=}"
          if [[ "$target_val" != "${hostTriple}"* ]]; then
            is_cross=1
            break
          fi
          ;;
        --target|-target)
          skip_next=1
          ;;
      esac
    done

    if [[ "$is_cross" -eq 1 ]]; then
      exec "${clangUnwrapped}/bin/clang++" -resource-dir "${clangResourceDir}" "$@"
    else
      exec "${llvmPackages.clang}/bin/clang++" "$@"
    fi
    EOF
    chmod +x $out/bin/clang++

    cat > $out/bin/clang-cl << 'EOF'
    #!/usr/bin/env bash
    exec "${clangUnwrapped}/bin/clang-cl" -resource-dir "${clangResourceDir}" "$@"
    EOF
    chmod +x $out/bin/clang-cl
  '';

  # ---------------------------------------------------------------------------
  # 2. Multi-Host `graphics-setup` Diagnostics & Driver Auto-Configuration CLI
  # ---------------------------------------------------------------------------
  graphics-setup = final.writeShellApplication {
    name = "graphics-setup";
    runtimeInputs =
      with final;
      lib.optionals isLinux [
        pciutils      # lspci
        udev          # udevadm
      ];
    checkPhase = "";
    text =
      builtins.readFile ./scripts/common.sh
      + builtins.readFile ./scripts/graphics-setup.sh;
  };
}
