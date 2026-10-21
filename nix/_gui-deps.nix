# Native dependencies for the two frontends, in one place.
#
# Underscore-prefixed so import-tree skips it: it is a plain function, not a flake-parts
# module. The dev shell and both package builds import it, so they cannot disagree about
# which GTK or which Wayland the frontends are built against. A disagreement there would
# show up as a difference in the benchmark numbers and be read as a property of the
# toolkit.
#
# The attribute names carry a suffix because `rec` binds tighter than `with`: an
# attribute called `wayland` would shadow `pkgs.wayland` inside its own definition and
# yield a list containing itself.
pkgs: rec {
  nativeBuildInputs = [ pkgs.pkg-config ];

  # Sets GSETTINGS_SCHEMA_DIR and the GIO module path. Without it a GTK binary starts
  # and dies the first time it wants a schema. Only candidate A needs it.
  gtkNativeBuildInputs = nativeBuildInputs ++ [ pkgs.wrapGAppsHook4 ];

  # Candidate A. gtk4-layer-shell is here for the portability check under a nested
  # wlroots compositor; mutter ignores it.
  gtkLibs = with pkgs; [
    gtk4
    gtk4-layer-shell
    glib
    pango
    cairo
    gdk-pixbuf
    graphene
  ];

  # Candidate B. libwayland and libxkbcommon are opened at runtime rather than linked,
  # so they have to be found through the loader path as well as through pkg-config.
  waylandLibs = with pkgs; [
    wayland
    wayland-protocols
    libxkbcommon
  ];

  # cosmic-text reads the fontconfig database to resolve a family name to a file, the
  # same way Pango does, so both candidates draw with the same fonts.
  fontLibs = with pkgs; [
    fontconfig
    freetype
  ];

  # What each candidate alone needs. Handing both of them the union would put GTK in
  # the raw candidate.s closure and make the two sizes meaningless.
  crateInputs = {
    glimt = gtkLibs ++ fontLibs;
    glimt-raw = waylandLibs ++ fontLibs;
  };

  # The dev shell builds both, so it gets the union.
  buildInputs = gtkLibs ++ waylandLibs ++ fontLibs;

  libraryPath = pkgs.lib.makeLibraryPath (waylandLibs ++ fontLibs);
}
