{ inputs, lib, ... }:
{
  imports = [ inputs.dendritic-base.flakeModules.default ];

  dendritic-base = {
    root = ../.;

    # The GTK frontend is what the dashboard is; the other is the control it is measured
    # against. Both are built, and this is the one `nix run` and the module install.
    rust.defaultCrate = "glimt";

    # Puts a cross linker on PATH so `just dist` can build the client for both
    # architectures from this machine.
    rust.release.targets = [
      "x86_64-unknown-linux-musl"
      "aarch64-unknown-linux-musl"
    ];

    # Without this each crate's check runs the whole workspace's tests, which means a
    # crate that depends on neither GTK nor Wayland builds both of them to test itself,
    # and fails in the sandbox for want of a compositor.
    rust.crates = lib.genAttrs [ "glimt-cal" "glimtctl" "glimt-bench" ] (crate: {
      extraArgs.cargoTestFlags = [
        "--package"
        crate
      ];
    });
  };
}
