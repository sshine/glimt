{ inputs, ... }:
{
  perSystem =
    {
      pkgs,
      lib,
      system,
      ...
    }:
    let
      gui = import ./_gui-deps.nix pkgs;

      # A dev shell stages each package's default output, and pkg-config files live in
      # the `dev` one, so without this every *-sys crate's build script fails with
      # "try setting PKG_CONFIG_PATH". A package build gets this from its buildInputs
      # and needs nothing.
      # closePropagation walks propagatedBuildInputs, which is what makes the
      # difference: gtk4.pc names a dozen further .pc files in its Requires line, and a
      # path built from the direct dependencies alone resolves none of them.
      pkgConfigPath = lib.concatStringsSep ":" (
        lib.concatMap (p: [
          "${lib.getDev p}/lib/pkgconfig"
          "${lib.getDev p}/share/pkgconfig"
        ]) (lib.closePropagation (map lib.getDev gui.buildInputs))
      );
    in
    {
      devshells.default = {
        packages = [
          # Regenerates Cargo.nix; see `just crate2nix`. Pinned by this flake rather than
          # taken from nixpkgs, so the generator and the Cargo.nix in the tree move together.
          inputs.crate2nix.packages.${system}.default
          # Reading the benchmark output by hand.
          pkgs.jq
          # The portability check runs the frontends under a nested wlroots compositor on
          # its headless backend, which is the only way to exercise layer-shell without
          # leaving GNOME and without taking over the screen.
          pkgs.sway
          # Injecting key presses and capturing the result inside that nested compositor.
          pkgs.wtype
          pkgs.grim
          # Inspecting which Wayland protocols the compositor of the day offers.
          pkgs.wayland-utils
        ]
        ++ gui.gtkNativeBuildInputs
        ++ gui.buildInputs;

        env = [
          {
            name = "PKG_CONFIG_PATH";
            value = pkgConfigPath;
          }
          {
            name = "LD_LIBRARY_PATH";
            value = gui.libraryPath;
          }
        ];
      };
    };
}
