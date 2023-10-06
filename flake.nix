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

    bomper = {
      url = "github:justinrubek/bomper";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
      imports = [
        ./lib.nix

        ./flake-parts/rust-toolchain.nix
        ./flake-parts/cargo.nix

        ./flake-parts/shells.nix
        ./flake-parts/ci.nix

        ./flake-parts/pre-commit.nix
        ./flake-parts/formatting.nix
        inputs.pre-commit-hooks.flakeModule
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
