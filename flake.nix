{
  description = "Development environment for ovim";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
    treefmt-nix,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };
      rooted = exec:
        builtins.concatStringsSep "\n"
        [
          ''REPO_ROOT="$(git rev-parse --show-toplevel)"''
          exec
        ];

      scripts = {
        dx = {
          exec = rooted ''$EDITOR "$REPO_ROOT"/flake.nix'';
          description = "Edit flake.nix";
        };
        rx = {
          exec = rooted ''$EDITOR "$REPO_ROOT"/Cargo.toml'';
          description = "Edit Cargo.toml";
        };
      };

      scriptPackages =
        pkgs.lib.mapAttrs
        (
          name: script:
            pkgs.writeShellApplication {
              inherit name;
              text = script.exec;
              runtimeInputs = script.deps or [];
            }
        )
        scripts;
      # Initialize crane for building Tauri app
      craneLib = (crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable.latest.default);

      # Build pnpm dependencies as a fixed-output derivation
      pnpmDeps = pkgs.stdenv.mkDerivation {
        name = "ovim-pnpm-deps";
        src = ./.;

        nativeBuildInputs = with pkgs; [pnpm nodejs cacert];

        dontBuild = true;
        dontFixup = true;

        installPhase = ''
          export HOME=$TMPDIR
          export PNPM_HOME=$TMPDIR
          export NODE_EXTRA_CA_CERTS=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
          export STORE_PATH=$(pnpm store path)

          pnpm install --frozen-lockfile --ignore-scripts

          mkdir -p $out/node_modules
          cp -r node_modules $out/
          chmod -R +w $out
        '';

        outputHashMode = "recursive";
        outputHash = "sha256-m2uGEBzRnOyWWoIfgIgmtPvfl4TP29xKIGsKirRF/8U=";
      };

      # Build frontend separately
      frontend = pkgs.stdenv.mkDerivation {
        name = "ovim-frontend";
        src = ./.;

        nativeBuildInputs = with pkgs; [pnpm nodejs cacert];

        buildPhase = ''
          export HOME=$TMPDIR
          export PNPM_HOME=$TMPDIR
          export NODE_EXTRA_CA_CERTS=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
          cp -r ${pnpmDeps}/node_modules ./node_modules
          chmod -R +w ./node_modules
          pnpm build
        '';

        installPhase = ''
          mkdir -p $out
          cp -r dist $out/
        '';
      };

      # Build the Tauri app
      # Note: We don't use cargo artifacts caching because Tauri's build.rs
      # generates files that need to be accessible during the main build
      ovim = craneLib.buildPackage {
        src = craneLib.path ./src-tauri;
        pname = "ovim";
        version = "0.0.24";

        # Disable dependency caching to avoid Tauri build.rs permission file issues
        cargoArtifacts = null;

        # Only build binaries, skip examples
        cargoExtraArgs = "--bins";

        buildInputs = with pkgs; [
          libiconv
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        # Copy pre-built frontend before cargo build
        preConfigure = ''
          # Copy the pre-built frontend dist folder to parent directory
          # Tauri expects it at ../dist relative to src-tauri
          mkdir -p ../dist
          cp -r ${frontend}/dist/* ../dist/
        '';

        # Tauri needs these environment variables
        TAURI_PRIVATE_KEY = "";
        TAURI_KEY_PASSWORD = "";
      };
    in {
      packages = {
        default = ovim;
        inherit ovim;
      };

      devShells.default = pkgs.mkShell {
        name = "dev";
        # Available packages on https://search.nixos.org/packages
        buildInputs = with pkgs;
          [
            alejandra # Nix
            nixd
            statix
            deadnix
            just
            rust-bin.stable.latest.default
            rust-bin.stable.latest.rust-analyzer
            pnpm
          ]
          ++ builtins.attrValues scriptPackages;
      };

      formatter = let
        treefmtModule = {
          projectRootFile = "flake.nix";
          programs = {
            alejandra.enable = true; # Nix formatter
            rustfmt.enable = true; # Rust formatter
          };
        };
      in
        treefmt-nix.lib.mkWrapper pkgs treefmtModule;
    });
}
