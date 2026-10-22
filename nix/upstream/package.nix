# Nixpkgs Upstream Package Derivation
# Target path in nixpkgs: pkgs/by-name/cr/crorust/package.nix
#
# When submitting to github.com/NixOS/nixpkgs:
# 1. Place this file in pkgs/by-name/cr/crorust/package.nix
# 2. Update version, rev, and compute hash using:
#    nix-prefetch-url --unpack https://github.com/adi1ahmad/crorust/archive/v<version>.tar.gz
# 3. Compute cargoHash with:
#    nix-build -A crorust (nix will output the expected cargoHash upon failure)
# 4. Add maintainer entry to maintainers/maintainer-list.nix if not already registered

{ lib
, stdenv
, rustPlatform
, fetchFromGitHub
, pkg-config
, makeWrapper
, zstd
, bzip2
, xz
, cargo
, rustc
, llvmPackages
, squashfsTools
, jdk17
, gradle
, android-tools
, darwin
}:

rustPlatform.buildRustPackage rec {
  pname = "crorust";
  version = "0.0.1";

  src = fetchFromGitHub {
    owner = "adi1ahmad";
    repo = "crorust";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # Update with release hash
  };

  cargoHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # Update with vendor hash

  cargoBuildFlags = [ "-p" "crorust" ];

  # Ensure artifacts are emitted to standard target/ for Nixpkgs cargo-install-hook
  CARGO_TARGET_DIR = "target";

  cargoTestFlags = [
    "-p" "crorust"
    "-p" "crorust-windows"
    "-p" "crorust-common"
  ];

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    zstd
    bzip2
    xz
  ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
    darwin.apple_sdk.frameworks.CoreFoundation
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  postInstall = ''
    wrapProgram $out/bin/crorust \
      --suffix PATH : ${lib.makeBinPath ([
        cargo
        rustc
        llvmPackages.clang
        llvmPackages.lld
        llvmPackages.llvm
        jdk17
        gradle
        android-tools
      ] ++ lib.optional (!stdenv.hostPlatform.isDarwin) squashfsTools)} \
      --set-default JAVA_HOME ${jdk17.home}
  '';

  meta = with lib; {
    description = "Builds rust mobile and desktop projects without heavyweight external toolchains";
    homepage = "https://crorust.com";
    changelog = "https://github.com/adi1ahmad/crorust/releases/tag/v${version}";
    license = with licenses; [ agpl3Only ];
    mainProgram = "crorust";
    platforms = platforms.unix;
    maintainers = with maintainers; [ ];
  };
}
