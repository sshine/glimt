# A single-file release of the dashboard for machines without Nix.
#
# The dashboard links GTK, so a static musl build of it is not a thing that exists. The
# AppImage keeps the dynamic links and carries the whole runtime closure instead: glibc,
# GTK, the GIO modules, the GSettings schemas and the gdk-pixbuf loaders.
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
      # A GNOME runtime closure carries translation catalogues for every language. The
      # dashboard's own strings are English and the calendar has no translated UI, so
      # the release copies do not need them.
      withoutLocales =
        package:
        package.overrideAttrs (old: {
          postInstall = (old.postInstall or "") + ''
            rm -rf "$out/share/locale"
          '';
        });

      # libglvnd only needs to lose X11 for the runtime GTK. Other build-time users,
      # xvfb in particular, still want a libGL with GLX, so this stays out of the
      # overlay.
      libglvndWayland = pkgs.libglvnd.overrideAttrs (old: {
        buildInputs = builtins.filter (
          p:
          !(builtins.elem p [
            pkgs.libx11
            pkgs.libxext
            pkgs.xorgproto
          ])
        ) old.buildInputs;
        configureFlags = (old.configureFlags or [ ]) ++ [ "--disable-x11" ];
        postFixup = "";
      });

      # The first pass only trims data; nixpkgs re-evaluates the packages above these
      # against the trimmed versions.
      trimmedPkgs = pkgs.extend (
        _final: prev: {
          glib = withoutLocales prev.glib;
          isocodes = withoutLocales prev.isocodes;
          gdk-pixbuf = withoutLocales prev.gdk-pixbuf;
          gsettings-desktop-schemas = withoutLocales prev.gsettings-desktop-schemas;
          xkeyboard-config = withoutLocales prev.xkeyboard-config;
        }
      );

      # The second pass gives the release GTK an X11-free dependency chain. The
      # replacements are exposed as separate attributes rather than overriding
      # cairo/pango/libxkbcommon globally: gtk3 and gjs are still in the Rust
      # toolchain's build closure and need the X11 versions.
      runtimePkgs = trimmedPkgs.extend (
        final: prev:
        let
          freetypeWayland = prev.freetype.overrideAttrs (old: {
            propagatedBuildInputs = builtins.filter (
              p:
              !(builtins.elem (p.pname or "") [
                "brotli"
                "bzip2"
              ])
            ) old.propagatedBuildInputs;
          });

          fontconfigWayland = prev.fontconfig.override {
            freetype = freetypeWayland;
          };

          harfbuzzWayland = prev.harfbuzz.override {
            freetype = freetypeWayland;
          };

          cairoWayland = prev.cairo.override {
            x11Support = false;
            xcbSupport = false;
            glib = prev.glib;
            freetype = freetypeWayland;
            fontconfig = fontconfigWayland;
          };

          pangoWayland = prev.pango.override {
            x11Support = false;
            cairo = cairoWayland;
            glib = prev.glib;
            harfbuzz = harfbuzzWayland;
          };

          # The X11 chain is libglvnd, libepoxy, cairo, pango and libxkbcommon.
          # Each has a different way of turning X11 off, and libepoxy ties EGL to
          # X11 in nixpkgs, so it gets a small custom build.
          libepoxyWayland =
            let
              epoxy =
                (prev.libepoxy.override {
                  x11Support = false;
                  libGL = libglvndWayland;
                }).overrideAttrs
                  (old: {
                    buildInputs = [ libglvndWayland ];
                    mesonFlags = [
                      "-Degl=yes"
                      "-Dglx=no"
                      "-Dx11=false"
                      "-Dtests=false"
                    ];
                    # libepoxy dlopens EGL rather than linking it, so the store path
                    # has to be in the binary for the closure to carry it.
                    env = (old.env or { }) // {
                      NIX_CFLAGS_COMPILE = ''-DLIBGL_PATH="${lib.getLib libglvndWayland}/lib"'';
                    };
                    doCheck = false;
                  });
            in
            # GTK asks for `libepoxy.override { inherit x11Support; }`. Answering
            # with the EGL-only build is the whole point of this package.
            epoxy // { override = _: epoxy; };

          libtiffWayland = (prev.libtiff.override { withLerc = false; }).overrideAttrs (old: {
            doCheck = false;
            propagatedBuildInputs = builtins.filter (
              p:
              !(builtins.elem (p.pname or "") [
                "libwebp"
                "lerc"
                "libdeflate"
                "xz"
                "zstd"
              ])
            ) old.propagatedBuildInputs;
            cmakeFlags = (old.cmakeFlags or [ ]) ++ [
              "-Dwebp=OFF"
              "-Dlerc=OFF"
              "-Dlibdeflate=OFF"
              "-Dlzma=OFF"
              "-Dzstd=OFF"
            ];
          });

          libxkbcommonWayland =
            (prev.libxkbcommon.override {
              xkeyboard_config = prev.xkeyboard-config;
              withWaylandTools = false;
            }).overrideAttrs
              (old: {
                doCheck = false;
                outputs = [
                  "out"
                  "dev"
                ];
                nativeBuildInputs = builtins.filter (p: (p.pname or "") != "xvfb") old.nativeBuildInputs;
                nativeCheckInputs = builtins.filter (p: (p.pname or "") != "xvfb") (old.nativeCheckInputs or [ ]);
                buildInputs = builtins.filter (
                  p:
                  !(builtins.elem p [
                    prev.libxcb
                    prev.libxml2
                  ])
                ) old.buildInputs;
                mesonFlags =
                  map (
                    flag: if lib.hasPrefix "-Dx-locale-root=" flag then "-Dx-locale-root=/etc/X11/locale" else flag
                  ) (old.mesonFlags or [ ])
                  ++ [
                    "-Denable-x11=false"
                    "-Denable-xkbregistry=false"
                    "-Denable-tools=false"
                    "-Denable-docs=false"
                  ];
              });
        in
        {
          inherit cairoWayland pangoWayland;

          gdk-pixbuf = withoutLocales (prev.gdk-pixbuf.override { libtiff = libtiffWayland; });

          # GTK's GStreamer media backend is most of the runtime closure, and the
          # dashboard never plays media. The calendar also has no file chooser that
          # searches recent files and prints nothing, so the tracker and CUPS branches
          # go too: tinysparql and cups are most of what remains. Vulkan and Broadway
          # are backends it never uses. The layer-shell binding has to be rebuilt
          # against the same GTK, or its own reference would pull the full GTK back in
          # next to the lean one.
          #
          # The wrap hook is graphical by default, which pulls gtk3 and librsvg back
          # in so that SVG icons and the SVG gdk-pixbuf loader are available. The
          # dashboard draws its own text and needs neither.
          gtk4 =
            (prev.gtk4.override {
              trackerSupport = false;
              cupsSupport = false;
              vulkanSupport = false;
              broadwaySupport = false;
              x11Support = false;
              xineramaSupport = false;
              cairo = cairoWayland;
              pango = pangoWayland;
              harfbuzz = harfbuzzWayland;
              gdk-pixbuf = final.gdk-pixbuf;
              libtiff = libtiffWayland;
              libepoxy = libepoxyWayland;
              libGL = libglvndWayland;
              libxkbcommon = libxkbcommonWayland;
            }).overrideAttrs
              (old: {
                mesonFlags = (old.mesonFlags or [ ]) ++ [ "-Dmedia-gstreamer=disabled" ];

                # nixpkgs only carries libdrm when the Vulkan backend is on, but the
                # Wayland DMA-BUF path checks for it either way.
                buildInputs =
                  builtins.filter (
                    p:
                    !(builtins.elem (p.pname or "") [
                      "gst-plugins-base"
                      "gst-plugins-bad"
                      "iso-codes"
                    ])
                  ) old.buildInputs
                  ++ [ prev.libdrm ];

                postInstall = (old.postInstall or "") + ''
                  rm -rf "$out/share/locale" "$out/share/gtk-4.0/emoji"
                  # The GIR XML stays: gtk4-layer-shell scans it when it generates
                  # its own introspection data. The typelibs are for language
                  # bindings at runtime, which a Rust binary does not load.
                  rm -rf "$out/lib/girepository-1.0"
                '';
              });

          gtk4-layer-shell = prev.gtk4-layer-shell.override { gtk4 = final.gtk4; };

          wrapGAppsHook4 = prev.wrapGAppsHook4.override { isGraphical = false; };
        }
      );

      cargoNix = runtimePkgs.callPackage ./_cargo-nix.nix { } { };
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

        # The default is gzip. xz is slower to build and to mount, but this is a
        # release artifact and the difference is several megabytes.
        squashfsArgs = [
          "-comp"
          "xz"
          "-Xbcj"
          "x86"
          "-Xdict-size"
          "100%"
        ];
      };

      # The runtime is the same one nix-appimage prepends, so its size is where the
      # squashfs begins.
      runtime = inputs.nix-appimage.packages.${system}.appimage-runtimes.appimage-type2-runtime;

      # nix-appimage packs the store paths whole, so data a package installs but the
      # program never opens stays in the image. glibc's locale data and gconv modules,
      # the GCC runtime, documentation, typelibs and a few RPATH-only dependencies are
      # all removable without touching the packages other builds link against.
      #
      # This is a post-processing step rather than another overlay because changing
      # these store paths in Nix would rebuild the world. Inside the AppImage the paths
      # are private, so trimming the files in place is safe.
      prunedAppImage =
        pkgs.runCommand "glimt-${version}.AppImage"
          {
            nativeBuildInputs = [
              pkgs.binutils
              pkgs.squashfsTools
            ];
          }
          ''
            offset=$(stat -Lc%s ${runtime})

            unsquashfs -no-progress -offset "$offset" -d tree ${appimage}
            chmod -R u+w tree

            find tree/nix/store -maxdepth 1 -type d -name '*-glibc-*' \
              -exec rm -rf '{}/share/i18n' '{}/share/locale' '{}/lib/gconv' \;

            # libtiffxx is the C++ API, which GTK does not use, and graphite2's only
            # binary is a test tool. They are the last users of libstdc++.
            find tree/nix/store -path '*libtiff*/lib/libtiffxx.so*' -delete
            find tree/nix/store -type d -path '*graphite2*/bin' -prune -exec rm -rf {} +

            if find tree/nix/store -type f \( -name '*.so*' -o -perm -u+x \) \
              -exec readelf -d {} + 2>/dev/null | grep -q 'libstdc++'; then
              echo "refusing to drop libstdc++: something still needs it" >&2
              exit 1
            fi

            find tree/nix/store -maxdepth 1 -type d -name '*-gcc-*-lib' \
              -exec sh -c '
                cd "$1/lib"
                find . -maxdepth 1 \( -type f -o -type l \) \
                  ! -name "libgcc_s.so*" -delete
              ' _ {} \;

            # libidn2 and libunistring are in the closure through an RPATH, not
            # through a NEEDED entry. Check that nothing else picked them up before
            # removing the store paths.
            find tree/nix/store -maxdepth 1 -type d \
              \( -name '*-libidn2-*' -o -name '*-libunistring-*' \) \
              -exec rm -rf {} +
            if find tree/nix/store -type f \( -name '*.so*' -o -perm -u+x \) \
              -exec readelf -d {} + 2>/dev/null | grep -Eq 'libidn2|libunistring'; then
              echo "refusing to drop libidn2/libunistring: something still needs them" >&2
              exit 1
            fi

            find tree/nix/store -type d \
              \( -name doc -o -name man -o -name info -o -name girepository-1.0 \) \
              \( -path '*/share/*' -o -path '*/lib/*' \) -prune -exec rm -rf {} +

            # stdenv strips debug sections but keeps .symtab. Shared libraries do not
            # need their static symbol table at runtime, and it is still a large part
            # of the closure.
            find tree -type f -name '*.so*' -exec strip --strip-unneeded {} + 2>/dev/null || true
            find tree -type f -perm -u+x -exec strip --strip-unneeded {} + 2>/dev/null || true

            mksquashfs tree $out \
              -no-progress -all-root -noappend \
              -offset "$offset" \
              -comp xz -Xbcj x86 -Xdict-size 100%
            dd if=${runtime} of=$out conv=notrunc status=none
            chmod 755 $out
          '';
    in
    {
      # AppImages are a Linux format; leave the attribute out on Darwin so evaluation
      # there does not go looking for a runtime that does not exist.
      packages = lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
        glimt-appimage = prunedAppImage;
      };
    };
}
