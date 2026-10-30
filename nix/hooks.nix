# The Nix lint steps, with the generated Cargo.nix left out.
#
# `Cargo.nix` is crate2nix output rather than hand-written source. Its top-level
# `crateConfig` is a lambda whose pattern names are unused until the file is applied,
# which deadnix reports as dead code, and nixfmt rewrites it into a shape that
# `crate2nix generate` does not produce. `nix/treefmt.nix` already excludes it from a
# tree walk; these overrides are for hk, which passes the staged files explicitly and
# so does not consult treefmt's glob exclusions on the pre-commit pass.
#
# Each step is a leaf in dendritic-base's option, so replacing one is a mkForce rather
# than a merge with the default.
{ ... }:
{
  perSystem =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      treefmt = lib.getExe config.treefmt.build.wrapper;
    in
    {
      dendritic-base.hooks.steps = {
        "pre-commit".treefmt = lib.mkForce {
          check = "${treefmt} --fail-on-change --no-cache --excludes Cargo.nix {{files}}";
          fix = "${treefmt} --excludes Cargo.nix {{files}}";
        };

        "pre-push".deadnix = lib.mkForce {
          glob = "*.nix";
          check = "${lib.getExe pkgs.deadnix} --fail --exclude Cargo.nix {{files}}";
        };
      };
    };
}
