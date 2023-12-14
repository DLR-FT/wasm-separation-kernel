{
  description = "Flake utils demo";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, ... }@inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenix = inputs.fenix.packages.${system};

        rust-release = "stable";
        rust-targets = [
          "x86_64-unknown-linux-gnu"
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-musl"
          "wasm32-unknown-unknown"
          "wasm32-wasi"
          "thumbv6m-none-eabi"
        ];
        toolchain = fenix.combine ([
          fenix.${rust-release}.cargo
          fenix.${rust-release}.clippy
          fenix.${rust-release}.rustc
          fenix.${rust-release}.rust-src
        ] ++ (builtins.map
          (target: fenix.targets.${target}.${rust-release}.rust-std)
          rust-targets));
      in
      rec {
        packages.lwsk =
          let
            lwskToml = builtins.fromTOML (builtins.readFile ./lwsk/Cargo.toml);
          in
          pkgs.rustPlatform.buildRustPackage {
            pname = lwskToml.package.name;
            version = lwskToml.package.version;
            src = ./lwsk;
            cargoLock = {
              lockFile = ./lwsk/Cargo.lock;
            };
          };

        packages.lwsk-no_std = packages.lwsk.overrideAttrs (_: { buildNoDefaultFeatures = true; });

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            toolchain
            fenix.rust-analyzer
            fenix.latest.rustfmt
            wabt

            # c example
            meson
            ninja
            pkgsCross.wasi32.stdenv.cc
            llvmPackages.lld
          ];
        };
      }
    );
}
