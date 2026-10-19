{ ... }:
{
  perSystem =
    { ... }:
    {
      treefmt = {
        programs.taplo.enable = true;

        settings.global.excludes = [
          "*.png"
          "LICENSE-*"
        ];
      };
    };
}
