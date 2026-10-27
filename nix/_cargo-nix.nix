# The crate2nix instance, in a plain flavour and a linting one.
#
# Shared between the builds and the lint check so the two cannot diverge. The linting
# flavour differs only in what it sets on the workspace's own crates, which is the whole
# point: every third-party dependency keeps the store path the plain flavour gave it, so
# linting costs the eight crates here rather than the four hundred below them.
#
# `buildRustCrate` drives rustc directly rather than through cargo, so linting is a matter
# of swapping the driver for clippy-driver rather than of running `cargo clippy`.
#
# Taking `pkgs` as an argument rather than closing over this flake's own is what lets the
# overlay build against a consumer's nixpkgs.
{
  lib,
  pkgs,
  ...
}:
let
  # What nixpkgs' defaultCrateOverrides does not already carry.
  #
  # The GTK stack is covered there down to gdk4-sys; the layer-shell binding, the three
  # crates that reach Wayland and xkbcommon through pkg-config, and the two frontends
  # themselves are not.
  nativeOverrides =
    p:
    let
      gui = import ./_gui-deps.nix p;
    in
    {
      gtk4-layer-shell-sys = _attrs: {
        nativeBuildInputs = [ p.pkg-config ];
        buildInputs = [
          p.gtk4-layer-shell
          p.gtk4
        ];
      };

      wayland-sys = _attrs: {
        nativeBuildInputs = [ p.pkg-config ];
        buildInputs = [ p.wayland ];
      };

      xkbcommon = _attrs: {
        nativeBuildInputs = [ p.pkg-config ];
        buildInputs = [ p.libxkbcommon ];
      };

      smithay-client-toolkit = _attrs: {
        nativeBuildInputs = [ p.pkg-config ];
        buildInputs = [ p.libxkbcommon ];
      };

      glimt = _attrs: {
        nativeBuildInputs = gui.gtkNativeBuildInputs;
        buildInputs = gui.crateInputs.glimt;
      };

      glimt-raw = _attrs: {
        nativeBuildInputs = gui.nativeBuildInputs;
        buildInputs = gui.crateInputs.glimt-raw;
        # winit and the wayland crates dlopen rather than link, so the rpath the fixup
        # phase derives does not cover them.
        postFixup = ''
          patchelf --add-rpath ${gui.libraryPath} $out/bin/glimt-raw
        '';
      };
    };

  # rust-overlay's toolchain where there is one, nixpkgs' own rustc where there is not.
  # This flake's pkgs carries `rust-bin`; the nixpkgs a consumer applies the overlay to
  # need not, and a project's pinned compiler is not a thing to force on them.
  toolchainOf =
    p:
    lib.optionalAttrs (p ? rust-bin) (
      let
        toolchain = p.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
      in
      {
        rustc = toolchain;
        cargo = toolchain;
        # Only consulted when useClippy is set, so naming it here costs the plain
        # flavour nothing and keeps both flavours on one toolchain.
        clippy = toolchain;
      }
    );

  # Read off the directory rather than listed, so a crate added later is linted without
  # anyone remembering to add it here. `members = ["crates/*"]` makes the directory the
  # authority on this already.
  members = builtins.attrNames (builtins.readDir ../crates);

  manifest = lib.importTOML ../Cargo.toml;

  # crate2nix does not emit rust-version, so CARGO_PKG_RUST_VERSION would come through
  # empty and clippy would lose the floor it suppresses suggestions against. Without it a
  # lint that cargo clippy holds back, because the API it suggests postdates the declared
  # minimum, fires here instead and the two disagree about the same source.
  msrv = manifest.workspace.package.rust-version;

  # `[lints] workspace = true` in a member manifest is resolved by cargo, not by crate2nix,
  # and crate2nix emits no lints at all. Nothing carries the table down to rustc unless it
  # is read here, so a check without this one is blind to exactly the rules the workspace
  # cared enough to write down.
  lints = manifest.workspace.lints or { };
in
{
  lint ? false,
}:
# `allFeatures` is bolted on beside crate2nix's own attributes rather than wrapped around
# them, so callers reach `workspaceMembers` exactly as the generated file offers it.
import ../Cargo.nix {
  inherit pkgs;
  buildRustCrateForPkgs =
    p:
    let
      base = p.buildRustCrate.override (
        {
          defaultCrateOverrides = p.defaultCrateOverrides // nativeOverrides p;
        }
        // toolchainOf p
      );
    in
    crate:
    base (
      crate
      // lib.optionalAttrs (lint && builtins.elem crate.crateName members) {
        useClippy = true;
        # The default cap of "allow" silences every lint including clippy's, which would
        # leave a check that passes because it looked at nothing.
        capLints = "forbid";
        rust-version = msrv;
        inherit lints;
        # Uncapping only lets the warnings through; this is what makes them fail.
        extraRustcOpts = (crate.extraRustcOpts or [ ]) ++ [
          "-D"
          "warnings"
        ];
      }
    );
}
// {
  # Everything `--all-features` would turn on for one workspace crate: its defaults plus
  # every feature its own manifest declares. crate2nix resolves a member's features from
  # its manifest and nothing else, while `just lint` and `just test` run under
  # `--all-features`.
  allFeatures =
    crate:
    [ "default" ]
    ++ builtins.attrNames ((lib.importTOML (../crates + "/${crate}/Cargo.toml")).features or { });
}
