{
  inputs,
  self,
  lib,
  ...
}: {
  imports = [];

  perSystem = {
    self',
    pkgs,
    lib,
    system,
    inputs',
    ...
  }: let
  in {
    thoenix.terraformConfigurations = {
      enable = true;

      configDirectory = ../terraform/configurations;
    };
  };
}
