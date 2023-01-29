{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    terranix.url = "github:terranix/terranix";
    nix-filter.url = "github:numtide/nix-filter";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
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
    ...
  }:
    flake-parts.lib.mkFlake {inherit self;} {
      systems = [ "x86_64-linux" "aarch64-linux" ];
      imports = [
        ./lib.nix
        ./flake-parts/cargo.nix
      ];

      flake = {
        templates = {
          lib = {
            path = ./examples/lib;
            description = "A project using the thoenix's lib to manage terraform configurations";
          };

          flake-module = {
            path = ./examples/flake-module;
            description = "A project using the thoenix's flake-parts module to manage terraform configurations";
          };
        };

        flakeModule = ./flake-parts/flake-module.nix;
        customOutputModule = ./custom-outputs.nix;
      };
    };
}
