# Prebuilt release artifacts, for consumers who want glimt without a Rust toolchain
# entering their build closure.
#
# The pinned version deliberately trails Cargo.toml: it names the last tag whose
# artifacts are published, not whatever the working tree happens to claim. Bump it
# once a tag's Release workflow has finished; the hashes are the ones the release
# publishes, so they can be checked against the .sha256 assets.
#
# The dashboard is an AppImage rather than a bare ELF: GTK cannot be statically
# linked, so the single file carries the runtime closure. The client is a static
# musl tarball. Systems absent from the hashes do not get the package, which keeps
# both off Darwin.
{ ... }:
{
  perSystem =
    {
      system,
      pkgs,
      lib,
      ...
    }:
    let
      release = {
        version = "0.4.0";

        appimageHashes = {
          "x86_64-linux" = "sha256-U6e+/zsjX5f4VCWK7ryPoCJ4a3aTipcTNai0TsSxyJ0=";
        };

        clientHashes = {
          "x86_64-linux" = "sha256-ZxJ/o/SHFwYMuMYsOSlt8g0Qjnx5ZHEf+eQF7SvcY2I=";
          "aarch64-linux" = "sha256-1EMMTayyhLVMmBeAlT38hWgcf8KSPSwv7AYH86GnYG4=";
        };
      };

      tag = "v${release.version}";

      arch = {
        "x86_64-linux" = "x86_64";
        "aarch64-linux" = "aarch64";
      };

      triple = {
        "x86_64-linux" = "x86_64-unknown-linux-musl";
        "aarch64-linux" = "aarch64-unknown-linux-musl";
      };

      appimage = pkgs.fetchurl {
        url = "https://github.com/sshine/glimt/releases/download/${tag}/glimt-${tag}-${arch.${system}}.AppImage";
        hash = release.appimageHashes.${system};
      };

      client = pkgs.fetchurl {
        url = "https://github.com/sshine/glimt/releases/download/${tag}/glimtctl-${tag}-${triple.${system}}.tar.gz";
        hash = release.clientHashes.${system};
      };
    in
    {
      packages =
        lib.optionalAttrs (release.appimageHashes ? ${system}) {
          glimt-bin =
            pkgs.runCommand "glimt-bin-${release.version}"
              {
                nativeBuildInputs = [ pkgs.makeWrapper ];

                meta = {
                  description = "A resident calendar dashboard (prebuilt AppImage)";
                  mainProgram = "glimt";
                  license = with lib.licenses; [
                    mit
                    asl20
                  ];
                  platforms = [ "x86_64-linux" ];
                  sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
                };
              }
              ''
                install -Dm755 ${appimage} $out/bin/glimt.AppImage
                makeWrapper $out/bin/glimt.AppImage $out/bin/glimt
              '';
        }
        // lib.optionalAttrs (release.clientHashes ? ${system}) {
          glimtctl-bin =
            pkgs.runCommand "glimtctl-bin-${release.version}"
              {
                meta = {
                  description = "The glimt client (prebuilt)";
                  mainProgram = "glimtctl";
                  license = with lib.licenses; [
                    mit
                    asl20
                  ];
                  platforms = lib.attrNames release.clientHashes;
                  sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
                };
              }
              ''
                tar -xzf ${client}
                install -Dm755 glimtctl $out/bin/glimtctl
              '';
        };
    };
}
