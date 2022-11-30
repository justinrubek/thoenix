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

    # builds the terranix configuration for each terraform configuration
    # and merges them into a single configuration derivation
    finalConfigurations = inputs.thoenix.lib.buildTerraformConfigurations {
      configDir = terraformConfigurationDirectory;
      configNames = inputs.thoenix.lib.determineSubdirNames {
        path = terraformConfigurationDirectory;
      };

      inherit pkgs system;
    };

    # we could get the configure names directly from `determineSubdirNames`, but
    # the output of `buildTerraformConfigurations` is a set, so we can just use attrNames
    terraformConfigurationNames = builtins.attrNames finalConfigurations;

    # rename all values from finalConfigurations to be prefixed with `terraformConfiguration_`
    # this allows them to be specified in the `packages` flake output
    terraformConfigurationOutput = let
      prefix = "terraformConfiguration_";
      reducer = l: r: l // {"${prefix}${r}" = finalConfigurations.${r};};
    in
      builtins.foldl' reducer {} terraformConfigurationNames;

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
    # this is an alternative to the above `packages` output
    # you can build these using `nix build .#terraformConfigurations.${system}.${name}`
    terraformConfigurationPackages = finalConfigurations;

    apps = let
      # shortcuts for running commands inside a writeShellScriptBin
      jq = pkgs.lib.getBin pkgs.jq;

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
