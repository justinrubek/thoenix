{...}: {
  perSystem = {
    inputs',
    lib,
    pkgs,
    ...
  }: let
    # "stable", "latest", "minimal", "complete"
    channel = "latest";
    fenix-channel = inputs'.fenix.packages.${channel};

    # rust targets
    fenix-targets = with inputs'.fenix.packages.targets;
      [
        x86_64-unknown-linux-gnu.${channel}.rust-std
        aarch64-unknown-linux-gnu.${channel}.rust-std
      ]
      ++ lib.optionals pkgs.stdenv.isDarwin [
        x86_64-apple-darwin.${channel}.rust-std
        aarch64-apple-darwin.${channel}.rust-std
      ];

    fenix-toolchain = inputs'.fenix.packages.combine ([
        fenix-channel.rustc
        fenix-channel.cargo
        fenix-channel.clippy
        fenix-channel.rust-analysis
        fenix-channel.rust-src
        fenix-channel.rustfmt
        fenix-channel.llvm-tools-preview
      ]
      ++ fenix-targets);
  in {
    packages = {
      rust-toolchain = fenix-toolchain;
    };
  };
}
