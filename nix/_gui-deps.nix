# Native dependencies for the GTK frontend, in one place.
#
# Underscore-prefixed so import-tree skips it: it is a plain function, not a flake-parts
# module. The dev shell and the package build import it, so they cannot disagree about
# which GTK the dashboard is built against.
pkgs: rec {
  nativeBuildInputs = [ pkgs.pkg-config ];

  # Sets GSETTINGS_SCHEMA_DIR and the GIO module path. Without it a GTK binary starts
  # and dies the first time it wants a schema.
  gtkNativeBuildInputs = nativeBuildInputs ++ [ pkgs.wrapGAppsHook4 ];

  # gtk4-layer-shell is here for the portability check under a nested wlroots
  # compositor; mutter ignores it.
  gtkLibs = with pkgs; [
    gtk4
    gtk4-layer-shell
    glib
    (pkgs.pangoWayland or pkgs.pango)
    (pkgs.cairoWayland or pkgs.cairo)
    gdk-pixbuf
    graphene
  ];

  # Pango reads the fontconfig database to resolve a family name to a file.
  fontLibs = with pkgs; [
    fontconfig
    freetype
  ];

  buildInputs = gtkLibs ++ fontLibs;

  libraryPath = pkgs.lib.makeLibraryPath fontLibs;
}
