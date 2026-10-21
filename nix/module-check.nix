# Proof that the NixOS module evaluates.
#
# A module is a pile of option definitions that nothing type-checks until a system is
# built against it, so a typo in an option path or a package that is not in the overlay
# goes unnoticed until someone rebuilds their machine. This evaluates a system with the
# module turned on and writes out the one string that depends on all of it: the command
# the service runs, which can only be produced if the options resolved and the packages
# exist.
#
# Evaluation only. Nothing here builds a frontend, let alone a system.
{ inputs, ... }:
{
  perSystem =
    {
      pkgs,
      system,
      ...
    }:
    {
      checks.module =
        let
          machine = inputs.nixpkgs.lib.nixosSystem {
            inherit system;

            modules = [
              inputs.self.nixosModules.default
              {
                services.glimt.enable = true;

                # The least a system needs before the module system will answer.
                boot.loader.grub.devices = [ "nodev" ];
                fileSystems."/" = {
                  device = "nodev";
                  fsType = "ext4";
                };
                system.stateVersion = "25.05";
              }
            ];
          };

          service = machine.config.systemd.user.services.glimt.serviceConfig;
          keys = machine.config.programs.dconf.profiles.user.databases;
        in
        # The store paths are discarded from the string: this is a proof that the
        # module evaluates, and evaluating it should not build a frontend.
        pkgs.writeText "glimt-module-check" (
          builtins.unsafeDiscardStringContext ''
            ${service.ExecStart}
            ${builtins.toJSON (map (database: database.settings) keys)}
          ''
        );
    };
}
