{ pkgs ? import <nixpkgs> { } }:

let
  crorust-unwrapped = pkgs.callPackage ./nix/package.nix { };
in
pkgs.callPackage ./nix/wrapper.nix {
  inherit crorust-unwrapped;
}
