# Every binary the workspace produces, built through crate2nix.
#
# This replaces what dendritic-base derives. The base builds each binary member with
# buildRustPackage, which is one derivation per binary holding a private copy of the
# whole dependency tree: four binaries means compiling chrono, serde, zbus and the GTK
# bindings four times over, and the lint check and the test run compile them twice more.
# crate2nix emits one derivation per crate instead, so the four binaries, the lints and
# the tests all reach the same store paths for everything they share.
#
# The base's per-crate escape hatch does not reach the native dependencies either:
# `extraArgs` is a flake-level option and `pkgs` does not exist at that level, so
# `buildInputs` has nowhere to come from. mkForce is the smallest answer.
{ ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    let
      cargoNix = pkgs.callPackage ./_cargo-nix.nix { } { };

      binaries = [
        "glimt"
        "glimt-raw"
        "glimtctl"
        "glimt-bench"
      ];

      built = lib.genAttrs binaries (crate: cargoNix.workspaceMembers.${crate}.build);
    in
    {
      packages = lib.mapAttrs (_: lib.mkForce) (built // { default = built.glimt; });
      checks = lib.mapAttrs (_: lib.mkForce) built;
    };

  # The base's overlay builds every binary member with plain cargo, which links neither
  # GTK nor Wayland, so the two frontends need one of their own. It is a second overlay
  # rather than a replacement because `flake.overlays.default` is the base's to define.
  flake.overlays.frontends =
    final: _prev:
    let
      cargoNix = final.callPackage ./_cargo-nix.nix { } { };
    in
    {
      glimt = cargoNix.workspaceMembers.glimt.build;
      glimt-raw = cargoNix.workspaceMembers.glimt-raw.build;
    };
}
