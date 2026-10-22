{ lib
, stdenv
, rustPlatform
, pkg-config
, zstd
, bzip2
, xz
, darwin
, android-tools ? null
}:

rustPlatform.buildRustPackage rec {
  pname = "crorust";
  version = "0.0.1";

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let
        baseName = baseNameOf (toString path);
      in
        # Exclude build targets, git directories, and local caches
        ! (type == "directory" && (
            baseName == "target" ||
            baseName == ".git" ||
            baseName == ".direnv" ||
            baseName == ".cache" ||
            lib.hasPrefix "result" baseName
          ));
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  # Build the main CLI binary 'crorust' from the crorust workspace package
  cargoBuildFlags = [ "-p" "crorust" ];

  # In .cargo/config.toml, target-dir is redirected to .cache/target for local developer convenience.
  # However, Nixpkgs' buildRustPackage setup hooks (cargoInstallPostBuildHook and cargoInstallHook)
  # strictly expect artifacts in the standard target/ directory.
  # Setting CARGO_TARGET_DIR here overrides the file config inside the isolated Nix build sandbox.
  CARGO_TARGET_DIR = "target";

  # In the Nix sandbox, apk tests require external ANDROID_HOME / android.jar which are unavailable.
  # We test the standalone crates that pass cleanly in an isolated environment.
  cargoTestFlags = [
    "-p" "crorust"
    "-p" "crorust-windows"
    "-p" "crorust-common"
  ];

  nativeBuildInputs = [
    pkg-config
  ];

  nativeCheckInputs = lib.optional (
    android-tools != null && lib.meta.availableOn stdenv.hostPlatform android-tools
  ) android-tools;

  preCheck = ''
    export HOME=$TMPDIR
  '';

  buildInputs = [
    zstd
    bzip2
    xz
  ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
    darwin.apple_sdk.frameworks.CoreFoundation
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  meta = with lib; {
    description = "Builds rust mobile and desktop projects without heavyweight external toolchains (crorust.com)";
    homepage = "https://crorust.com";
    changelog = "https://github.com/adi1ahmad/crorust/releases/tag/v${version}";
    license = with licenses; [ agpl3Only ];
    mainProgram = "crorust";
    platforms = platforms.unix;
    maintainers = with maintainers; [ ];
  };
}
