{
  inputs,
  self,
  ...
}: {
  perSystem = {
    pkgs,
    lib,
    system,
    inputs',
    self',
    ...
  }: let
    # packages required for building the rust packages
    extraPackages =
      [
        pkgs.pkg-config
        pkgs.openssl
        pkgs.openssl.dev
      ]
      ++ lib.optionals pkgs.stdenv.isDarwin [
        pkgs.libiconv
        pkgs.darwin.apple_sdk.frameworks.AppKit
        pkgs.darwin.apple_sdk.frameworks.CoreFoundation
        pkgs.darwin.apple_sdk.frameworks.CoreServices
        pkgs.darwin.apple_sdk.frameworks.Foundation
        pkgs.darwin.apple_sdk.frameworks.Security
      ];
    withExtraPackages = base: base ++ extraPackages;

    craneLib = inputs.crane.lib.${system}.overrideToolchain self'.packages.rust-toolchain;

    commonArgs = rec {
      src = inputs.nix-filter.lib {
        root = ../.;
        include = [
          "crates"
          "Cargo.toml"
          "Cargo.lock"
        ];
      };

      pname = "thoenix";

      nativeBuildInputs = withExtraPackages [];
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath nativeBuildInputs;
    };

    cargoArtifacts = craneLib.buildDepsOnly commonArgs;

    packages = {
      default = packages.cli;
      cli = craneLib.buildPackage ({
          pname = "thoenix";
          inherit cargoArtifacts;
          cargoExtraArgs = "--bin thoenix";
          meta.mainProgram = "thoenix";
        }
        // commonArgs);

      cargo-doc = craneLib.cargoDoc ({
          inherit cargoArtifacts;
        }
        // commonArgs);
    };

    checks = {
      clippy = craneLib.cargoClippy (commonArgs
        // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-features -- --deny warnings";
        });

      rust-fmt = craneLib.cargoFmt (commonArgs
        // {
          inherit (commonArgs) src;
        });

      rust-tests = craneLib.cargoNextest (commonArgs
        // {
          inherit cargoArtifacts;
          partitions = 1;
          partitionType = "count";
          cargoExtraArgs = "--exclude annapurna-wasm --exclude annapurna-ui --workspace";
        });
    };
  in rec {
    inherit packages checks;

    legacyPackages = {
      cargoExtraPackages = extraPackages;
    };
  };
}
