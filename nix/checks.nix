# Clippy and the test suite, as checks rather than devshell commands.
#
# One derivation per crate, through the same crate2nix instance the binaries are built
# with, rather than one `cargo clippy` and one `cargo test` over a vendored workspace.
# Dependencies are the store paths the build already made, so these cost the eight crates
# in `crates/` and nothing underneath them.
#
# `runTests` is what reaches the test targets: it wires the dev-dependencies in and
# compiles `tests/*.rs` alongside the lib, which is what `--all-targets` means to the
# devshell's `just lint`. The two checks differ only in `testCrateFlags`, which the runner
# reads and the build does not, so listing the tests here and running them there is one
# compile and two runs of it.
{ ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    let
      cargoNix = pkgs.callPackage ./_cargo-nix.nix { } { lint = true; };

      forEachMember =
        testCrateFlags:
        lib.mapAttrsToList (
          crate: member:
          member.build.override {
            features = cargoNix.allFeatures crate;
            runTests = true;
            inherit testCrateFlags;
          }
        ) cargoNix.workspaceMembers;

      # Naming the list in the environment is what makes it build; nothing is read back.
      collect =
        name: description: crates:
        pkgs.runCommand name
          {
            inherit crates;
            meta = { inherit description; };
          }
          ''
            for crate in $crates; do
              echo "${name}: $crate"
            done
            touch $out
          '';
    in
    {
      checks = {
        clippy = collect "glimt-clippy" "Clippy over the glimt workspace" (forEachMember [ "--list" ]);
        test = collect "glimt-test" "The glimt test suite" (forEachMember [ ]);
      };
    };
}
