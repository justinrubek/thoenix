{
  inputs,
  self,
  ...
} @ part-inputs: {
  imports = [];

  perSystem = {
    self',
    pkgs,
    lib,
    system,
    inputs',
    ...
  }: let
    # the providers to be available for terraform
    # see "nixpkgs/pkgs/applications/networking/cluster/terraform-providers/providers.json"
    terraformPluginsPredicate = p: [
      # p.aws
      # p.kubernetes
      # p.nomad
      p.null
      # p.local
      # p.random
      # p.template
      # p.tls
      # p.tfe
      # p.vault
    ];
    terraform = pkgs.opentofu.withPlugins terraformPluginsPredicate;
  in rec {
    packages = {
      # expose terraform with the pinned providers
      inherit terraform;
    };

    apps = let
      jq = pkgs.lib.getExe pkgs.jq;

      # print the list of pinnable terraform providers from nixpkgs
      terraform-provider-pins = pkgs.writeShellScriptBin "terraform-provider-pins" ''
        cat ${inputs.nixpkgs}/pkgs/applications/networking/cluster/terraform-providers/providers.json
      '';
    in {
      printTerraformProviders = {
        type = "app";
        program = pkgs.lib.getExe terraform-provider-pins;
      };
    };
  };
}
