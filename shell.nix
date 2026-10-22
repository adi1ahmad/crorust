{ pkgs ? import <nixpkgs> { } }:

let
  crorust = import ./default.nix { inherit pkgs; };
in
pkgs.mkShell {
  packages = [
    crorust
    pkgs.cargo
    pkgs.rustc
    pkgs.llvmPackages.clang
    pkgs.llvmPackages.lld
    pkgs.llvmPackages.llvm
    pkgs.pkg-config
  ] ++ pkgs.lib.optional pkgs.stdenv.hostPlatform.isLinux pkgs.squashfsTools;
}
