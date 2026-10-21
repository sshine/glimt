# The two frontends, replacing what dendritic-base derives for them.
#
# The base builds every binary member with plain cargo, which links neither GTK nor
# Wayland. Its per-crate escape hatch does not reach here either: `extraArgs` is a
# flake-level option and `pkgs` does not exist at that level, so `buildInputs` has
# nowhere to come from. mkForce is the smallest answer; every other crate in the
# workspace still comes from the base.
{ ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    let
      frontend = name: pkgs.callPackage ./_frontend-package.nix { crate = name; };

      frontends = lib.genAttrs [ "glimt" "glimt-raw" ] frontend;
    in
    {
      packages = lib.mapAttrs (_: lib.mkForce) frontends;
      checks = lib.mapAttrs (_: lib.mkForce) frontends;
    };

  # The base's overlay builds every binary member with plain cargo, which links neither
  # GTK nor Wayland, so the two frontends need one of their own. It is a second overlay
  # rather than a replacement because `flake.overlays.default` is the base's to define.
  flake.overlays.frontends = final: _prev: {
    glimt = final.callPackage ./_frontend-package.nix { crate = "glimt"; };
    glimt-raw = final.callPackage ./_frontend-package.nix { crate = "glimt-raw"; };
  };
}
