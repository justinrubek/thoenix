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
      systems = ["x86_64-linux" "aarch64-linux"];
      imports = [
        ./lib.nix
      ];

      flake.customOutputModule = ./custom-outputs.nix;
    };
}
