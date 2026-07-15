# Nix installation

Flake users only. Add wayshot as an input:

```nix
{
  inputs.wayshot.url = "github:waycrate/wayshot";
}
```

- [Package overlay](#package-overlay)
- [Home Manager module](#home-manager-module)

## Package overlay

Overlay for manual management of wayshot

```nix
{ inputs, pkgs, ... }:
{
  nixpkgs.overlays = [ inputs.wayshot.overlays.default ];

  environment.systemPackages = [ pkgs.wayshot ];
  # or in home-manager:
  # home.packages = [ pkgs.wayshot ];
}
```

Build features can be toggled via override (defaults match the crate's
default feature set, see [package.nix](./package.nix) for the full list):

```nix
pkgs.wayshot.override {
  webpSupport = false;
  avifSupport = false;
}
```

## Home Manager module

Import the module in any of your home-manager config files:

```nix
{ inputs, ... }:
{
  imports = [
    inputs.wayshot.homeModules.default
  ];
  ...
}
```

### Configuration

```nix
{
  programs.wayshot = {
    enable = true;
    # Build features ignored when `package` is set.
    features = {
      jxlSupport = false;
    };
    settings = {
      base = {
        cursor = true;
        clipboard = false;
        file = true;
      };
      file = {
        path = "~/Pictures";
      };
    };
  };
}
```
