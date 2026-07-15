{self}: {
  config,
  options,
  lib,
  pkgs,
  ...
}: let
  cfg = config.programs.wayshot;
  inherit (pkgs.stdenv.hostPlatform) system;
  inherit (lib) types;
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkOption mkEnableOption;

  tomlFormat = pkgs.formats.toml {};
in {
  options.programs.wayshot = {
    enable = mkEnableOption "wayshot, a screenshot tool for wlroots based compositors";

    package = mkOption {
      description = "The package to use for `wayshot`";
      default = self.packages.${system}.default.override cfg.features;
      defaultText = "wayshot from this flake, with `features` applied";
      type = types.package;
    };

    features = mkOption {
      description = ''
        Build-feature arguments passed to the default package via `.override`,
        e.g. `{ webpSupport = false; }` (see nix/package.nix for the full list).
        Only takes effect while `package` is left at its default.
      '';
      default = {};
      type = types.attrsOf types.bool;
    };

    settings = mkOption {
      description = "Configuration written to wayshot's `config.toml`.";
      default = {};
      type = tomlFormat.type;
      example = {
        base = {
          cursor = true;
          clipboard = true;
        };
        file = {
          path = "~/Pictures";
        };
      };
    };
  };

  config = mkIf cfg.enable {
    warnings =
      lib.optional
      (cfg.features != {} && options.programs.wayshot.package.highestPrio < (lib.mkOptionDefault null).priority)
      "programs.wayshot.features is ignored because programs.wayshot.package is set explicitly.";

    home.packages = [cfg.package];
    xdg.configFile."wayshot/config.toml" = mkIf (cfg.settings != {}) {
      source = tomlFormat.generate "config.toml" cfg.settings;
    };
  };
}
