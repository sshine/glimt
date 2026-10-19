# Build definition shared by both spikes. Not a flake-parts module; import-tree skips
# `_`-prefixed basenames.
#
# Takes `pkgs` through callPackage rather than closing over this flake's own, so the
# same file could build against a consumer's nixpkgs later.
{
  pkgs,
  lib,
  rustPlatform,
  crate,
}:
let
  manifest = lib.importTOML (../crates + "/${crate}/Cargo.toml");
  gui = import ./_gui-deps.nix pkgs;
in
rustPlatform.buildRustPackage {
  pname = crate;
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../README.md
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs =
    if crate == "spike-gtk4" then gui.gtkNativeBuildInputs else gui.nativeBuildInputs;
  buildInputs = gui.crateInputs.${crate};

  cargoBuildFlags = [
    "--package"
    crate
  ];

  # Both spikes need a compositor to do anything, and the sandbox has none.
  doCheck = false;

  # winit and the wayland crates dlopen rather than link, so the rpath the fixup phase
  # derives does not cover them.
  postFixup = ''
    patchelf --add-rpath ${gui.libraryPath} $out/bin/${crate}
  '';

  meta = {
    inherit (manifest.package) description;
    mainProgram = crate;
    license = with lib.licenses; [
      mit
      asl20
    ];
    platforms = lib.platforms.linux;
  };
}
