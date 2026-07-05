# pkl-lsp

<!-- simit:badges:start -->

![CI](https://img.shields.io/badge/CI-managed--extra-2088ff) [![Nix](https://img.shields.io/badge/Nix-managed-5277c3)](flake.nix) [![docs](https://img.shields.io/badge/docs-enabled-6f42c1)](https://docs.rs/pkl-lsp-core) [![crates.io](https://img.shields.io/badge/crates.io-ready-f46623)](https://crates.io/crates/pkl-lsp-core)

<!-- simit:badges:end -->

Rust language server for [Pkl](https://pkl-lang.org/) with native and WASM
editor targets.

This repository intentionally vendors `pklr` while WASM-capable evaluator IO
support is prepared for upstreaming. The vendored crate is temporary and should
be retired once equivalent support is released upstream.

## Workspace

- `pkl-lsp-core` - parser-backed diagnostics, symbols, imports, hover,
  completion, definition, references, and archived indexes.
- `pkl-lsp-server` - native stdio LSP server.
- `pkl-lsp-wasm` - WASM bridge for VS Code web.
- `pkl-lsp-vscode` - VS Code/Codium extension sources.
- `vendor/pklr` - temporary upstream-compatible `pklr` work area.

## Development

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

The native server is:

```sh
cargo run -p pkl-lsp-server -- --stdio
```
