{ lib
, symlinkJoin
, makeWrapper
, crorust-unwrapped
, cargo
, rustc
, llvmPackages
, squashfsTools ? null
, jdk17 ? null
, kotlin ? null
, gradle ? null
, enableGradle ? false
, android-tools ? null
, stdenv
}:

let
  runtimeDeps = [
    cargo
    rustc
    llvmPackages.clang
    llvmPackages.lld
    llvmPackages.llvm
  ]
  ++ lib.optional (jdk17 != null) jdk17
  ++ lib.optional (kotlin != null) kotlin
  ++ lib.optional (enableGradle && gradle != null) gradle
  ++ lib.optional (android-tools != null) android-tools
  # squashfsTools is included on Linux as an optional multi-threaded performance accelerator,
  # but is optional because crorust-linux includes a pure-Rust fallback.
  ++ lib.optional (!stdenv.hostPlatform.isDarwin && squashfsTools != null) squashfsTools;
in
symlinkJoin {
  name = "crorust-${crorust-unwrapped.version}";
  paths = [ crorust-unwrapped ];
  nativeBuildInputs = [ makeWrapper ];
  postBuild = ''
    wrapProgram $out/bin/crorust \
      --suffix PATH : ${lib.makeBinPath runtimeDeps} \
      ${lib.optionalString (jdk17 != null) "--set-default JAVA_HOME ${jdk17.home}"}
  '';
  meta = crorust-unwrapped.meta;
}
