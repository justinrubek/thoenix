{
  inputs,
  self,
  lib,
  ...
}: {
  imports = [ ];

  perSystem = {
    self',
    pkgs,
    lib,
    system,
    inputs',
    ...
  }: let
    # individual terraform configurations are stored in the `terraform/configuration` directory
    # and are referenced by their name in the configuration directory
    terraformConfigurationDirectory = ../terraform/configurations;
    terraformConfigurationNames = inputs.thoenix.lib.determineSubdirNames {
      path = terraformConfigurationDirectory;
    };

    # builds the terranix configuration for each terraform configuration
    # and merges them into a single configuration derivation
    finalConfigurations = inputs.thoenix.lib.buildTerraformConfigurations {
      configDir = terraformConfigurationDirectory;
      configNames = terraformConfigurationNames;
      inherit pkgs system;
    };

    # rename all values from finalConfigurations to be prefixed with `terraformConfiguration_`
    # this allows them to be specified in the `packages` flake output
    terraformConfigurationOutput = let
      prefix = "terraformConfiguration_";
      reducer = l: r: l // {"${prefix}${r}" = finalConfigurations.${r};};
    in
      builtins.foldl' reducer {} (builtins.attrNames finalConfigurations);

    # create a JSON file containing the names of configurations as well as paths to their directories
    # { configurations = [ { name = "core"; path = "/nix/store/..."; } ]; }
    configurationMatrix = let
      reducer = l: r:
        l
        // {
          configurations =
            l.configurations
            ++ [
              {
                name = r;
                path = finalConfigurations.${r};
              }
            ];
        };
      configurationJSON = builtins.toJSON (builtins.foldl' reducer {configurations = [];} terraformConfigurationNames);
    in
      pkgs.stdenv.mkDerivation {
        name = "terraform-configuration-matrix";
        buildCommand = ''
          mkdir -p $out
          echo '${configurationJSON}' > $out/terraform-configuration-matrix.json
        '';
      };
  in rec {
    packages =
      {
        # expose a package containing a JSON list of configuration names and their paths
        terraformConfigurationMatrix = configurationMatrix;
      }
      # expose all configurations as packages
      // terraformConfigurationOutput;

    # expose all configuration packages, but using a custom output to avoid namespacing
    terraformConfigurationPackages = finalConfigurations;

    apps = let
      # shortcuts for running commands inside a writeShellScriptBin
      jq = "${pkgs.jq}/bin/jq";

      generate-matrix-names = pkgs.writeShellScriptBin "generate-terraform-matrix" ''
        # access the 'name' key of each configuration
        cat ${packages.terraformConfigurationMatrix}/terraform-configuration-matrix.json | ${jq} -r '.configurations' | ${jq} 'map(.name)'
      '';
    in {
      # output a list of available terraform configuration names
      generateTerraformMatrix = {
        type = "app";
        program = pkgs.lib.getExe generate-matrix-names;
      };
    };
  };
}
