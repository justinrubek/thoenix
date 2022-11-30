{
  config,
  lib,
  flake-parts-lib,
  ...
}: let
  inherit
    (lib)
    mkOption
    types
    ;
  inherit
    (flake-parts-lib)
    mkTransposedPerSystemModule
    ;
in
  mkTransposedPerSystemModule {
    name = "terraformConfigurationPackages";
    option = mkOption {
      type = types.lazyAttrsOf types.package;
      default = {};
      description = ''
        An attribute set of Terraform configuration packages.
        The contents will be valid terraform HCL/JSON files containing the terraform configuration merged with the terranix configuration.
      '';
    };
    # this refers to the current file
    file = ./custom-outputs.nix;
  }
