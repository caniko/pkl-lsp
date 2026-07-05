# PKL LSP

Rust-backed language support for [Pkl](https://pkl-lang.org/) in VS Code,
VSCodium, and compatible Open VSX clients.

## Features

- Diagnostics for malformed Pkl documents.
- Document symbols for imports, declarations, classes, type aliases, and annotations.
- Hover, completion, go-to-definition, and references for deterministic local symbols.
- Desktop extension with a bundled native `pkl-lsp` server.
- Web extension path backed by the `pkl-lsp-wasm` bundle.

## Configuration

`pklLsp.serverPath` can point to a custom `pkl-lsp` binary. Leave it empty to
use the bundled server for the current platform.

## Current Scope

This first release focuses on parser-backed editor intelligence and WASM
packaging. Full evaluator-backed workspace parity depends on the temporary
vendored `pklr` capability work being upstreamed.

## Repository

Source, issues, and release notes live at
<https://codeberg.org/caniko/pkl-lsp>.
