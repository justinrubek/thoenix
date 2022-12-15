{
  inputs,
  self,
  lib,
  ...
}: {
  imports = [];

  flake.lib = rec {
    # call into terranix to build a configuration, loading all `.nix` files in the directory as modules
    mkTerranixConfiguration = {
      path,
      system,
      terranixModules ? [],
      ...
    }@attrs: let
      # pass unused arguments to terranix
      extraAttrs = builtins.removeAttrs attrs [ "path" "system" ];

      filterFileName = name: lib.strings.hasSuffix ".nix" name;

      # filter out all directories and files that are not .nix files
      directoryContents = builtins.readDir path;
      filesOnly = lib.attrsets.filterAttrs (x: y: y == "regular") directoryContents;
      nixFiles = builtins.attrNames (lib.attrsets.filterAttrs (x: y: filterFileName x) filesOnly);

      modules = map (x: "${path}/${x}") nixFiles;
    in
      inputs.terranix.lib.terranixConfiguration {
        inherit system;

        modules = terranixModules ++ modules;
        # support manually specifying null values. without this terranix will remove keys with a null value
        # strip_nulls = true;
      };

    # find all `.tf` and `.tf.json` files in the directory and create a list of paths to them
    # includes all subdirectories so they can be used as terraform modules
    mkTerraformConfiguration = {path}: let
      # filter out all directories and files that are not .tf or .tf.json files
      directoryContents = builtins.readDir path;
      filesOnly = lib.attrsets.filterAttrs (x: y: y == "regular") directoryContents;
      filterFileName = name: lib.strings.hasSuffix ".tf" name || lib.strings.hasSuffix ".tf.json" name;
      tfFiles = builtins.attrNames (lib.attrsets.filterAttrs (x: y: filterFileName x) filesOnly);

      includedFiles = map (x: "${path}/${x}") tfFiles;
      subdirectories = builtins.attrNames (lib.attrsets.filterAttrs (x: y: y == "directory") directoryContents);
      includedDirectories = map (x: "${path}/${x}") subdirectories;
    in (includedFiles ++ includedDirectories);

    # combine the terranix and terraform configurations into a single derivation containing all of the files
    # intended for the final build output for terraform.
    # the output is devoid of any nix-specific code
    # bundles the generated terranix configurations with existing `.tf` and `.tf.json` files from their configuration directory
    mkTerraformConfigurationPackage = {
      name,
      path,
      pkgs,
      system,
      generatedConfig = self.lib.mkTerranixConfiguration {inherit path system;};
      terranixModules ? [],
      ...
    }@attrs: let
      # pass unused arguments to mkTerranixConfiguration
      extraAttrs = builtins.removeAttrs attrs [ "name" "path" "pkgs" "system" ];

      generatedConfig = self.lib.mkTerranixConfiguration {inherit path system terranixModules; } // extraAttrs;
      providedConfig = self.lib.mkTerraformConfiguration {inherit path;};
    in
      pkgs.runCommandNoCC "terraform-config-${name}" {} ''
        mkdir -p $out
        # copy the provided (terraform) configuration files
        ${lib.strings.concatStringsSep "\n" (map (x: "cp -r ${x} $out") providedConfig)}
        # copy the generated (terranix) configuration file
        cp ${generatedConfig} $out/config.tf.json
      '';

    # helper to create and bundle multiple configurations
    buildTerraformConfigurations = {
      configDir,
      configNames,
      system,
      pkgs,
      terranixModules ? [],
    }: let
      # reducer should provide an output containing the configurations keyed by name
      reducer = l: r: let
        path = "${configDir}/${r}";
        configuration = self.lib.mkTerraformConfigurationPackage {
          name = r;
          inherit path pkgs system terranixModules;
        };
      in
        {
          "${r}" = configuration;
        }
        // l;
    in
      builtins.foldl' reducer {} configNames;

    determineSubdirNames = {path}: let
      directoryContents = builtins.readDir path;
      subdirectories = builtins.attrNames (lib.attrsets.filterAttrs (x: y: y == "directory") directoryContents);
    in
      subdirectories;
  };
}
