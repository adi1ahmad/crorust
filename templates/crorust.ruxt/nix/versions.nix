# =============================================================================
# VERSION MANAGEMENT - Single Source of Truth for Toolchain Versions
# =============================================================================
# Kept strictly synchronized with manifest.toml and rust-toolchain.toml.
# =============================================================================
{ pkgs }:
let
  llvmMajor = "19";
  llvmPackages = pkgs."llvmPackages_${llvmMajor}";
in
{
  inherit llvmMajor llvmPackages;
  llvmLib = llvmPackages.llvm.lib;

  # Pinned JDK 17 LTS matching manifest.toml (`jvm_target: "17"`) and crorust's
  # standalone Android bytecode compiler (`AndroidBytecodeCompiler`).
  jdk = pkgs.jdk17;

  # Android SDK levels matching manifest.toml
  android = {
    minSdk = "21";
    targetSdk = "34";
    compileSdk = "35";
    buildTools = "34.0.0";
  };
}