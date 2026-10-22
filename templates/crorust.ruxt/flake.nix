{
  description = "🦀 crorust.ruxt — Hermetic Multi-Host Cross-Platform Rust Development Environment";

  nixConfig = {
    accept-flake-config = true;
    extra-experimental-features = [ "nix-command" "flakes" ];
    log-level = "error";
    warn-dirty = false;
    max-jobs = "auto";
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    crorust = {
      url = "path:../..";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, crorust }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          rust-overlay.overlays.default
          (import ./nix/overlays.nix)
          (final: prev:
            let
              toolchain = final.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
              modernRustPlatform = final.makeRustPlatform {
                cargo = toolchain;
                rustc = toolchain;
              };
            in
            {
              crorust-unwrapped = (crorust.overlays.default final prev).crorust-unwrapped.override {
                rustPlatform = modernRustPlatform;
              };
            }
          )
        ];

        pkgs = import nixpkgs {
          inherit system overlays;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        };

        inherit (pkgs.stdenv) hostPlatform;

        versions = import ./nix/versions.nix { inherit pkgs; };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Compose a minimal Android SDK (API 34 only) on supported hosts (x86_64-linux, Darwin).
        # Note: Google does not publish native aarch64-linux Android SDK build-tools in nixpkgs;
        # on aarch64-linux, androidSdk is null and crorust manages android.jar/r8 hermetically.
        androidSdk =
          if (hostPlatform.isLinux && hostPlatform.isx86_64) || hostPlatform.isDarwin then
            (pkgs.androidenv.composeAndroidPackages {
              buildToolsVersions = [ versions.android.buildTools ];
              platformVersions = [ versions.android.compileSdk ];
              includeNDK = false;
              includeEmulator = false;
              includeSystemImages = false;
            }).androidsdk
          else
            null;

        # Use crorust-unwrapped built via our overlay (using rust-overlay's modern rustc)
        # so crorust invokes the devShell's rust-overlay toolchain at runtime.
        crorustPkg = pkgs.crorust-unwrapped;

        dependencies = import ./nix/dependencies.nix {
          inherit pkgs versions;
        };

        environment = import ./nix/environment.nix {
          inherit pkgs dependencies versions androidSdk;
        };

        # Helper to construct clean flake apps (`nix run .#<name>`)
        mkFlakeApp = name: scriptText: {
          type = "app";
          program = "${pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = dependencies.core ++ dependencies.build ++ dependencies.mobile ++ dependencies.web ++ [
              rustToolchain
              crorustPkg
            ];
            text = scriptText;
          }}/bin/${name}";
        };

      in
      {
        # =====================================================================
        # DEVELOPMENT SHELLS (`nix develop`, `nix develop .#minimal`, etc.)
        # =====================================================================
        devShells = {
          # 1. Default Shell: Full Desktop + Mobile (Android/iOS) + Web (WASM)
          default = pkgs.mkShell ({
            name = "crorust-ruxt-dev";

            # Disable Nix cc-wrapper hardening flags (-fzero-call-used-regs=used-gpr,
            # _FORTIFY_SOURCE, etc.) which break wasm32-unknown-unknown and Android
            # cross-compilation targets (nixpkgs issue #101979).
            hardeningDisable = [ "all" ];

            buildInputs =
              dependencies.core
              ++ dependencies.graphics
              ++ dependencies.mobile;

            nativeBuildInputs =
              dependencies.build
              ++ dependencies.web
              ++ [
                rustToolchain
                crorustPkg
                pkgs.graphics-setup
              ];

            shellHook =
              builtins.readFile ./nix/scripts/common.sh
              + builtins.readFile ./nix/scripts/shell-hook.sh;
          } // environment);

          # 2. Minimal Shell: Instant startup for pure Rust coding & cargo check
          minimal = pkgs.mkShell {
            name = "crorust-ruxt-minimal";
            hardeningDisable = [ "all" ];

            buildInputs = dependencies.core ++ dependencies.graphics;
            nativeBuildInputs = dependencies.build ++ [
              rustToolchain
              crorustPkg
            ];

            LD_LIBRARY_PATH = environment.LD_LIBRARY_PATH or "";
            PKG_CONFIG_PATH = environment.PKG_CONFIG_PATH or "";

            shellHook = ''
              echo -e "\033[1;36m🦀 crorust.ruxt minimal shell loaded\033[0m (\033[2m$(rustc --version)\033[0m)"
              echo -e "\033[2m💡 Run 'nix develop' for full Android/Web/Graphics diagnostics\033[0m"
            '';
          };

          # 3. CI Shell: Headless, silent environment with Lavapipe software Vulkan for snapshot tests
          ci = pkgs.mkShell ({
            name = "crorust-ruxt-ci";
            hardeningDisable = [ "all" ];

            buildInputs = dependencies.core ++ dependencies.graphics;
            nativeBuildInputs = dependencies.build ++ dependencies.web ++ [
              rustToolchain
              crorustPkg
            ];

            NIX_SHELL_SILENT = "1";
            CI = "1";

            shellHook =
              builtins.readFile ./nix/scripts/common.sh
              + builtins.readFile ./nix/scripts/shell-hook.sh;
          } // environment);

          # 4. Debug Shell: Includes heavy GPU profilers (RenderDoc, vkcube, glxgears) on demand
          debug = pkgs.mkShell ({
            name = "crorust-ruxt-debug";
            hardeningDisable = [ "all" ];

            buildInputs =
              dependencies.core
              ++ dependencies.graphics
              ++ dependencies.mobile
              ++ dependencies.debugTools;

            nativeBuildInputs =
              dependencies.build
              ++ dependencies.web
              ++ [
                rustToolchain
                crorustPkg
                pkgs.graphics-setup
              ];

            shellHook =
              builtins.readFile ./nix/scripts/common.sh
              + builtins.readFile ./nix/scripts/shell-hook.sh;
          } // environment);
        };

        # =====================================================================
        # FLAKE APPS (`nix run`, `nix run .#info`, `nix run .#ci`, etc.)
        # =====================================================================
        apps = rec {
          default = info;

          i = mkFlakeApp "crorust-i" ''
            export PATH="${pkgs.crorust-llvm-bridge}/bin:$PATH"
            exec crorust i "$@"
          '';

          info = mkFlakeApp "crorust-info" ''
            export PATH="${pkgs.crorust-llvm-bridge}/bin:$PATH"
            exec crorust i "$@"
          '';

          check = mkFlakeApp "crorust-check" ''
            export PATH="${pkgs.crorust-llvm-bridge}/bin:$PATH"
            echo "🔍 Checking host target..."
            cargo check --all-targets
            echo "🌐 Checking WebAssembly target (wasm32-unknown-unknown)..."
            cargo check --target wasm32-unknown-unknown --no-default-features --lib
            echo "🤖 Checking Android target (aarch64-linux-android)..."
            cargo check --target aarch64-linux-android --lib
            echo "✅ All cross-platform targets typechecked cleanly!"
          '';

          ci = mkFlakeApp "crorust-ci" ''
            export PATH="${pkgs.crorust-llvm-bridge}/bin:$PATH"
            exec bash ./nix/scripts/ci.sh "$@"
          '';

          android = mkFlakeApp "crorust-android" ''
            export PATH="${pkgs.crorust-llvm-bridge}/bin:$PATH"
            exec crorust build --platform android "$@"
          '';

          web = mkFlakeApp "crorust-web" ''
            exec trunk serve "$@"
          '';

          graphics-setup = {
            type = "app";
            program = "${pkgs.graphics-setup}/bin/graphics-setup";
            meta = {
              description = "CroRust graphics & multi-host GPU diagnostics utility";
            };
          };
        };

        packages = {
          default = crorustPkg;
          crorust = crorustPkg;
          graphics-setup = pkgs.graphics-setup;
        };

        formatter = pkgs.nixfmt-rfc-style or pkgs.nixpkgs-fmt;
      }
    );
}
