{ inputs, ... }:
{
  imports = [ inputs.dendritic-base.flakeModules.default ];

  dendritic-base = {
    root = ../.;

    # Puts `pkgs.walltime` in reach of the dev shell. An overlay is a function rather
    # than a package, so it is a fact this level can hold.
    extraOverlays = [ inputs.walltime.overlays.default ];

    # Several members produce a binary and none is named after the repository, so the
    # default cannot be derived. glimtctl is the one a user would run by hand.
    rust.defaultCrate = "glimtctl";

    # Each crate's check otherwise runs the whole workspace's tests, which puts the two
    # spikes' display-dependent tests in the way of the calendar crate's.
    rust.crates.glimt-cal.extraArgs.cargoTestFlags = [
      "--package"
      "glimt-cal"
    ];
  };
}
