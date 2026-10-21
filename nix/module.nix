# A NixOS module, so installing glimt is one import rather than a service file and a
# dconf incantation.
#
# The module does the two things that are not obvious from the outside: it keeps the
# daemon running for the life of the graphical session, because a dashboard that has to
# start before it can appear is not the thing this is, and it binds a key to it.
#
# Binding the key is where GNOME makes a choice for you. There are two ways and the
# module offers both:
#
#   - A dconf custom keybinding running `glimtctl toggle`. Declarative, which is why it
#     is the default, and it needs nothing from the user on first run. The catch is that
#     GNOME keeps custom keybindings in one list: a database set here is a default, and
#     a user who has ever added a custom shortcut through Settings has a list of their
#     own in ~/.config/dconf/user that shadows it. If the key does nothing, that is why.
#
#   - The GlobalShortcuts portal, which glimt registers itself when `portal` is set.
#     GNOME asks once, in a dialog, and remembers. It is less declarative and it is the
#     only one of the two whose key press carries an activation token, which is what
#     mutter wants before it will hand a window the keyboard. Worth switching to if the
#     window comes up without focus.
{ inputs, ... }:
{
  flake.nixosModules.default =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      cfg = config.services.glimt;

      # GNOME names a custom keybinding by the dconf path it lives at, and the path has
      # to appear in the list as well as hold the settings.
      path = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/glimt/";

      arguments = [
        "--months"
        (toString cfg.months)
      ]
      ++ lib.optionals cfg.portal [
        "--trigger"
        cfg.trigger
      ]
      ++ lib.optional (!cfg.portal) "--no-portal"
      ++ cfg.extraArgs;
    in
    {
      options.services.glimt = {
        enable = lib.mkEnableOption "the glimt dashboard";

        package = lib.mkOption {
          type = lib.types.package;
          default = pkgs.glimt;
          defaultText = lib.literalExpression "pkgs.glimt";
          description = ''
            The frontend to run. `pkgs.glimt` is the GTK one and what you want;
            `pkgs.glimt-raw` is the same dashboard drawn without a toolkit, kept as the
            control the GTK one is measured against.
          '';
        };

        controlPackage = lib.mkOption {
          type = lib.types.package;
          default = pkgs.glimtctl;
          defaultText = lib.literalExpression "pkgs.glimtctl";
          description = "The client the keybinding runs, and that you can drive by hand.";
        };

        months = lib.mkOption {
          type = lib.types.ints.positive;
          default = 6;
          description = ''
            Months in the overview. Six is the half year the reference calendar prints.
            A count that divides the year aligns to its block, so three gives the
            quarter and twelve the year.
          '';
        };

        shortcut = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = "<Super>Return";
          example = "<Super>k";
          description = ''
            The key that shows the dashboard, in GNOME's own notation. Null leaves the
            keyboard alone, which is what you want alongside `portal`.

            Not Super+space, which looks like the obvious choice and is GNOME's own
            binding for switching input sources: on a machine with two keyboard layouts
            it is already taken.
          '';
        };

        portal = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = ''
            Register the shortcut through the GlobalShortcuts portal instead of through
            dconf. GNOME asks once and remembers. Slower to set up and the only one of
            the two whose key press carries an activation token.
          '';
        };

        trigger = lib.mkOption {
          type = lib.types.str;
          default = "SUPER+Return";
          description = ''
            The shortcut to ask the portal for, in the portal's notation rather than
            GNOME's. A preference: the portal's own settings win.
          '';
        };

        extraArgs = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          example = [ "--offline" ];
          description = "Anything else to put on the daemon's command line.";
        };
      };

      config = lib.mkIf cfg.enable {
        # Both: the first carries glimtctl, the second the two frontends, which need
        # native inputs the first knows nothing about. Order matters.
        nixpkgs.overlays = [
          inputs.self.overlays.default
          inputs.self.overlays.frontends
        ];

        environment.systemPackages = [
          cfg.package
          cfg.controlPackage
        ];

        # Bound to the graphical session rather than to login: the daemon talks Wayland
        # and has nothing to do before there is a compositor to talk to.
        systemd.user.services.glimt = {
          description = "glimt dashboard";
          documentation = [ "https://github.com/sshine/glimt" ];

          partOf = [ "graphical-session.target" ];
          after = [ "graphical-session.target" ];
          wantedBy = [ "graphical-session.target" ];

          serviceConfig = {
            ExecStart = "${lib.getExe cfg.package} ${lib.escapeShellArgs arguments}";
            Restart = "on-failure";
            RestartSec = 2;
            Slice = "session.slice";
          };
        };

        programs.dconf.enable = true;

        programs.dconf.profiles.user.databases = lib.mkIf (cfg.shortcut != null) [
          {
            settings = {
              "org/gnome/settings-daemon/plugins/media-keys".custom-keybindings = [ path ];

              "org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/glimt" = {
                name = "glimt";
                command = "${lib.getExe cfg.controlPackage} toggle";
                binding = cfg.shortcut;
              };
            };
          }
        ];
      };
    };
}
