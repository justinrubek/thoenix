{inputs, ...}: {
  perSystem = {
    config,
    pkgs,
    system,
    inputs',
    self',
    lib,
    ...
  }: let
    inherit (self'.packages) rust-toolchain;
    inherit (self'.legacyPackages) cargoExtraPackages ciPackages;

    devTools = [
      # rust tooling
      rust-toolchain
      pkgs.cargo-audit
      pkgs.cargo-udeps
      pkgs.cargo-nextest
      pkgs.bacon
      # formatting
      self'.packages.treefmt
      # misc
      pkgs.opentofu
    ];
  in {
    devShells = {
      default = pkgs.mkShell rec {
        packages = devTools ++ cargoExtraPackages ++ ciPackages;

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath packages;
        RUST_SRC_PATH = "${self'.packages.rust-toolchain}/lib/rustlib/src/rust/src";

        shellHook = ''
          ${config.pre-commit.installationScript}
        '';
      };
    };
  };
}
