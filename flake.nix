{
  description = "crorust: Cross-platform build tool for Rust desktop & mobile apps (crorust.com)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        };
        inherit system;
      });
    in
    {
      overlays.default = final: prev: {
        crorust-unwrapped = final.callPackage ./nix/package.nix { };
        crorust = final.callPackage ./nix/wrapper.nix {
          crorust-unwrapped = final.crorust-unwrapped;
        };
      };

      packages = forAllSystems ({ pkgs, ... }: {
        default = pkgs.crorust;
        crorust = pkgs.crorust;
        crorust-unwrapped = pkgs.crorust-unwrapped;
      });

      apps = forAllSystems ({ pkgs, ... }: {
        default = {
          type = "app";
          program = "${pkgs.crorust}/bin/crorust";
          meta = pkgs.crorust.meta;
        };
        crorust = {
          type = "app";
          program = "${pkgs.crorust}/bin/crorust";
          meta = pkgs.crorust.meta;
        };
      });

      devShells = forAllSystems ({ pkgs, ... }: {
        default = pkgs.mkShell {
          packages = [
            pkgs.crorust
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
            pkgs.llvmPackages.clang
            pkgs.llvmPackages.lld
            pkgs.llvmPackages.llvm
            pkgs.pkg-config
            pkgs.jdk17
            pkgs.kotlin
            pkgs.android-tools
          ] ++ pkgs.lib.optional pkgs.stdenv.hostPlatform.isLinux pkgs.squashfsTools;
        };
      });
    };
}
