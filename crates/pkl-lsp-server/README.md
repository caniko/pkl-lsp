# pkl-lsp-server

Native stdio Language Server Protocol server for PKL. It serves the
parser-backed features from `pkl-lsp-core` to desktop editor clients.

Install the binary with:

```sh
cargo install pkl-lsp-server
```

Run it as an LSP stdio server:

```sh
pkl-lsp --stdio
```

The server reads and writes framed JSON-RPC messages on standard input and
output. See [docs.rs] for the package API and metadata.

[docs.rs]: https://docs.rs/pkl-lsp-server
