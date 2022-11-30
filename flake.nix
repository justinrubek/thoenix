{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    terranix.url = "github:terranix/terranix";
  };

  outputs = {
    self,
    flake-utils,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit self;} {
      imports = [
        ./lib.nix
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
