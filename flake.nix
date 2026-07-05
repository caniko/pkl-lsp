{
  description = "pkl-lsp — Rust/WASM language server and editor plugin for PKL";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    git-hooks.url = "github:cachix/git-hooks.nix";
    rs-harbor = {
      url = "git+https://codeberg.org/caniko/rs-harbor.git";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crane.follows = "crane";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-utils.follows = "flake-utils";
    };
    rs-harbor-macos-sdk-pin.url = "git+ssh://git@codeberg.org/caniko/rs-harbor-macos-sdk-pin.git";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    crane,
    flake-utils,
    treefmt-nix,
    git-hooks,
    rs-harbor,
    rs-harbor-macos-sdk-pin,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      toolchain = rs-harbor.lib.mkToolchain {
        inherit pkgs;
        channel = "stable";
        extensions = ["rustfmt" "clippy"];
        withRustAnalyzer = false;
        crossTargets = [
          "x86_64-unknown-linux-gnu"
          "aarch64-unknown-linux-gnu"
          "x86_64-pc-windows-gnu"
          "x86_64-apple-darwin"
          "aarch64-apple-darwin"
          "wasm32-unknown-unknown"
        ];
      };
      inherit (toolchain) rustToolchain craneLib;
      cross = rs-harbor.lib.mkCross {
        inherit pkgs system;
        macosSdkStorePath = rs-harbor-macos-sdk-pin.storePath;
        macosSdkOutputHash = rs-harbor-macos-sdk-pin.outputHash;
        osxSdkVersion = rs-harbor-macos-sdk-pin.sdkVersion;
      };
      src = craneLib.cleanCargoSource ./.;
      commonArgs = {
        inherit src;
        pname = "pkl-lsp";
        version = "0.1.0";
        strictDeps = true;
      };
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      serverCommonArgs =
        commonArgs
        // {
          cargoExtraArgs = "-p pkl-lsp-server";
        };
      serverPackages = rs-harbor.lib.mkCrossPackages {
        inherit pkgs cross;
        inherit (toolchain) craneLib;
        pname = "pkl-lsp-server";
        commonArgs = serverCommonArgs;
        targets = [
          "native"
          "aarch64-linux"
          "windows"
          "darwin-x86_64"
          "darwin-aarch64"
        ];
        targetArgs = {
          aarch64-linux.doCheck = false;
          windows = {
            doCheck = false;
            nativeBuildInputs = [cross.mingwBinutils];
          };
          darwin-x86_64.doCheck = false;
          darwin-aarch64.doCheck = false;
        };
      };
      package = serverPackages.pkl-lsp-server;
      wasmArtifacts = craneLib.buildDepsOnly (commonArgs
        // {
          cargoExtraArgs = "-p pkl-lsp-wasm";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        });
      wasmCheck = craneLib.cargoBuild (commonArgs
        // {
          cargoArtifacts = wasmArtifacts;
          cargoExtraArgs = "-p pkl-lsp-wasm";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        });
      vscodeExtensionSource = pkgs.stdenvNoCC.mkDerivation {
        pname = "pkl-lsp-vscode-extension-source";
        version = "0.1.0";
        src = ./pkl-lsp-vscode;
        installPhase = ''
          mkdir -p "$out"
          cp -R . "$out/"
        '';
      };
      treefmtEval = treefmt-nix.lib.evalModule pkgs (import ./nix/treefmt.nix);
      pre-commit-check = git-hooks.lib.${system}.run {
        src = ./.;
        hooks = import ./nix/pre-commit.nix {
          inherit pkgs;
          treefmtWrapper = treefmtEval.config.build.wrapper;
          inherit rustToolchain;
        };
      };
    in {
      packages = {
        default = package;
        pkl-lsp = package;
        pkl-lsp-server-linux-x64 = package;
        pkl-lsp-server-linux-arm64 = serverPackages.pkl-lsp-server-aarch64-linux;
        pkl-lsp-server-win32-x64 = serverPackages.pkl-lsp-server-windows;
        pkl-lsp-server-darwin-x64 = serverPackages.pkl-lsp-server-darwin-x86_64;
        pkl-lsp-server-darwin-arm64 = serverPackages.pkl-lsp-server-darwin-aarch64;
        vscode-extension-source = vscodeExtensionSource;
      };
      formatter = treefmtEval.config.build.wrapper;
      checks = {
        default = package;
        pkl-lsp-server-linux-x64 = package;
        pkl-lsp-server-linux-arm64 = serverPackages.pkl-lsp-server-aarch64-linux;
        pkl-lsp-server-win32-x64 = serverPackages.pkl-lsp-server-windows;
        pkl-lsp-server-darwin-x64 = serverPackages.pkl-lsp-server-darwin-x86_64;
        pkl-lsp-server-darwin-arm64 = serverPackages.pkl-lsp-server-darwin-aarch64;
        wasm = wasmCheck;
        formatting = treefmtEval.config.build.check self;
        clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets --all-features -- --deny warnings";
          });
        fmt = craneLib.cargoFmt {
          inherit src;
          pname = "pkl-lsp";
        };
      };
      devShells.default = craneLib.devShell {
        checks = self.checks.${system};
        packages = with pkgs;
          [
            cargo-about
            cargo-audit
            cargo-cyclonedx
            cargo-deny
            cargo-llvm-cov
            cargo-sbom
            cargo-nextest
            binaryen
            cosign
            jq
            minisign
            nodejs
            wasm-pack
            pre-commit
            rpm
            debootstrap
            util-linux
            reprepro
            rust-analyzer
            taplo
          ]
          ++ pre-commit-check.enabledPackages;
        shellHook = pre-commit-check.shellHook;
      };
      apps.local-check-fast = {
        type = "app";
        program = let
          script = pkgs.writeShellApplication {
            name = "local-check-fast";
            runtimeInputs = with pkgs; [
              cargo-deny
              git
              jq
              rustToolchain
            ];
            text = ''
              set -euo pipefail
              cargo test --workspace --all-features
              cargo clippy --workspace --all-targets --all-features -- --deny warnings
              cargo deny check bans licenses sources
              cargo package --workspace --allow-dirty --list >/dev/null
            '';
          };
        in "${script}/bin/local-check-fast";
        meta.description = "Run fast local validation checks";
      };
      apps.package-vsix = {
        type = "app";
        program = let
          script = pkgs.writeShellApplication {
            name = "package-vsix";
            runtimeInputs = with pkgs; [
              clang
              coreutils
              nix
              nodejs
              rustToolchain
              wasm-pack
              binaryen
              lld
            ];
            text = ''
              set -euo pipefail

              detect_target() {
                case "$(uname -s)-$(uname -m)" in
                  Linux-x86_64) echo linux-x64 ;;
                  Linux-aarch64|Linux-arm64) echo linux-arm64 ;;
                  Darwin-x86_64) echo darwin-x64 ;;
                  Darwin-arm64) echo darwin-arm64 ;;
                  MINGW64_NT-*-x86_64|MSYS_NT-*-x86_64|CYGWIN_NT-*-x86_64) echo win32-x64 ;;
                  MINGW64_NT-*-aarch64|MSYS_NT-*-aarch64|CYGWIN_NT-*-aarch64) echo win32-arm64 ;;
                  *)
                    echo "unsupported packaging host: $(uname -s)-$(uname -m)" >&2
                    exit 2
                    ;;
                esac
              }

              current_target="$(detect_target)"
              targets="''${PKL_LSP_VSIX_TARGETS:-''${*:-$current_target}}"

              server_attr_for_target() {
                case "$1" in
                  linux-x64) echo pkl-lsp-server-linux-x64 ;;
                  linux-arm64) echo pkl-lsp-server-linux-arm64 ;;
                  darwin-x64) echo pkl-lsp-server-darwin-x64 ;;
                  darwin-arm64) echo pkl-lsp-server-darwin-arm64 ;;
                  win32-x64) echo pkl-lsp-server-win32-x64 ;;
                  *)
                    echo "unsupported VS Code target '$1'" >&2
                    exit 2
                    ;;
                esac
              }

              server_binary_for_target() {
                case "$1" in
                  win32-*) echo pkl-lsp.exe ;;
                  *) echo pkl-lsp ;;
                esac
              }

              wasm-pack build crates/pkl-lsp-wasm --target web --out-dir ../../pkl-lsp-vscode/media --no-opt
              wasm-opt --enable-bulk-memory -Oz pkl-lsp-vscode/media/pkl_lsp_wasm_bg.wasm -o pkl-lsp-vscode/media/pkl_lsp_wasm_bg.wasm
              cd pkl-lsp-vscode
              npm ci
              npm run compile
              rm -rf bin
              for target in $targets; do
                attr="$(server_attr_for_target "$target")"
                binary="$(server_binary_for_target "$target")"
                server_out="$(nix build --no-link --print-out-paths "..#$attr")"
                server_path="$server_out/bin/$binary"
                if [ ! -x "$server_path" ]; then
                  echo "server binary for $target does not exist or is not executable: $server_path" >&2
                  exit 1
                fi

                mkdir -p "bin/$target"
                cp "$server_path" "bin/$target/$binary"
                chmod 0755 "bin/$target/$binary"
                npx vsce package --no-dependencies --target "$target"
                rm -rf "bin/$target"
              done
            '';
          };
        in "${script}/bin/package-vsix";
        meta.description = "Build the VS Code and VSCodium extension package";
      };
      apps.local-check-release = {
        type = "app";
        program = let
          script = pkgs.writeShellApplication {
            name = "local-check-release";
            runtimeInputs = with pkgs; [
              cargo-about
              cargo-cyclonedx
              cargo-deny
              cargo-sbom
              cosign
              jq
              minisign
              rustToolchain
            ];
            text = ''
              set -euo pipefail
              version="''${1:-}"
              if [ -z "$version" ]; then
                echo "usage: local-check-release <version>" >&2
                exit 2
              fi
              ${self.apps.${system}.local-check-fast.program}
              mkdir -p release
              if [ -f about-template.hbs ]; then
                cargo about generate --output-file release/THIRD_PARTY_LICENSES.html about-template.hbs
              else
                echo "warning: about-template.hbs not found; skipping cargo-about report" >&2
              fi
              cargo sbom --output-format cyclone_dx_json_1_5 > "release/''${version}.cdx.json"
              cargo sbom --output-format spdx_json_2_3 > "release/''${version}.spdx.json"
              if [ -n "''${COSIGN_PRIVATE_KEY:-}" ]; then
                echo "COSIGN_PRIVATE_KEY present; local release parity will not sign or upload" >&2
              else
                echo "warning: keyless Sigstore and COSIGN_PRIVATE_KEY unavailable locally; skipping local cosign signing" >&2
              fi
              echo "local release parity dry-run passed for ''${version}; no external publish was attempted"
            '';
          };
        in "${script}/bin/local-check-release";
        meta.description = "Run local release parity checks without publishing";
      };
      apps.local-release-deploy = {
        type = "app";
        program = let
          script = pkgs.writeShellApplication {
            name = "local-release-deploy";
            runtimeInputs = with pkgs; [
              git
              jq
            ];
            text = ''
              set -euo pipefail
              version="''${1:-}"
              publish_flag="''${2:-}"
              publish_version="''${3:-}"
              if [ -z "$version" ] || [ "$publish_flag" != "--publish" ] || [ "$publish_version" != "$version" ]; then
                echo "usage: local-release-deploy <version> --publish <version>" >&2
                echo "refusing to publish without an explicit matching confirmation" >&2
                exit 2
              fi
              echo "local-release-deploy is a project-specific hook; add publisher steps before using it" >&2
              exit 2
            '';
          };
        in "${script}/bin/local-release-deploy";
        meta.description = "Run the guarded local release deployment hook";
      };
    });
}
