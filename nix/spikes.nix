# The two spike binaries, replacing what dendritic-base derives for them.
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
      spike = name: pkgs.callPackage ./_spike-package.nix { crate = name; };

      spikes = lib.genAttrs [ "spike-gtk4" "spike-raw" ] spike;
    in
    {
      packages = lib.mapAttrs (_: lib.mkForce) spikes;
      checks = lib.mapAttrs (_: lib.mkForce) spikes;
    };
}
