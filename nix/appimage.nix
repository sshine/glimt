# A single-file release of the dashboard for machines without Nix.
#
# The frontends link GTK and Wayland, so a static musl build of them is not a thing that
# exists. The AppImage keeps the dynamic links and carries the whole runtime closure
# instead: glibc, GTK, the GIO modules, the GSettings schemas and the gdk-pixbuf loaders.
# nix-appimage mounts the squashfs with user namespaces and makes the closure visible at
# /nix/store, so the wrapper's absolute store paths still resolve on the host.
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
      # GTK's GStreamer media backend is most of the runtime closure, and the dashboard
      # never plays media. Turn it off for the release build. The layer-shell binding has
      # to be rebuilt against the same GTK, or its own reference would pull the full GTK
      # back in next to the lean one.
      releasePkgs = pkgs.extend (
        final: prev: {
          gtk4 = prev.gtk4.overrideAttrs (old: {
            mesonFlags = (old.mesonFlags or [ ]) ++ [ "-Dmedia-gstreamer=disabled" ];
            buildInputs = builtins.filter (
              p:
              !(builtins.elem (p.pname or "") [
                "gst-plugins-base"
                "gst-plugins-bad"
              ])
            ) old.buildInputs;
          });

          gtk4-layer-shell = prev.gtk4-layer-shell.override { gtk4 = final.gtk4; };
        }
      );

      cargoNix = releasePkgs.callPackage ./_cargo-nix.nix { } { };
      glimt = cargoNix.workspaceMembers.glimt.build;
      version = glimt.version;

      # nix-appimage finds the .desktop file beside the entrypoint and uses it for the
      # AppImage metadata. The dashboard is a shortcut daemon, so it stays out of menus.
      desktop = pkgs.writeText "glimt.desktop" ''
        [Desktop Entry]
        Type=Application
        Name=glimt
        Comment=A resident calendar dashboard
        Exec=glimt
        Icon=view-calendar
        Terminal=false
        Categories=Utility;Calendar;
        NoDisplay=true
      '';

      # A root with the binary and the desktop file, rather than the package itself, so
      # adding release metadata does not recompile the workspace.
      entrypoint = pkgs.runCommand "glimt-appimage-entrypoint-${version}" { } ''
        mkdir -p $out/bin $out/share/applications
        ln -s ${lib.getExe glimt} $out/bin/glimt
        install -Dm644 ${desktop} $out/share/applications/glimt.desktop
      '';

      appimage = inputs.nix-appimage.lib.${system}.mkAppImage {
        pname = "glimt";
        name = "glimt-${version}.AppImage";
        program = "${entrypoint}/bin/glimt";
      };
    in
    {
      # AppImages are a Linux format; leave the attribute out on Darwin so evaluation
      # there does not go looking for a runtime that does not exist.
      packages = lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
        glimt-appimage = appimage;
      };
    };
}
