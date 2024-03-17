{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";

    terranix.url = "github:terranix/terranix";
    thoenix = {
      url = "github:justinrubek/thoenix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        terranix.follows = "terranix";
      };
    };

    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    flake-utils,
    flake-parts,
    pre-commit-hooks,
    ...
  } @ inputs:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux"];
      imports = [
        pre-commit-hooks.flakeModule
        inputs.thoenix.customOutputModule
        ./flake-parts
      ];
    };
}
