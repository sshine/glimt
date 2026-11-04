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
        version = "0.2.1";

        appimageHashes = {
          "x86_64-linux" = "sha256-Nq7iUKpCPz75Ni2wPlaiiV7MDgBi56miWrKxbxhrhOI=";
        };

        clientHashes = {
          "x86_64-linux" = "sha256-HNZHlqTxRV23c95c/XPMJprtJDJVCYmVnlOXkEtYalA=";
          "aarch64-linux" = "sha256-NZU2VUSrCBx8jmEf4dyvtDHimV5h6O1V4eZ2/rHWIcc=";
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
