# pkl-lsp-wasm

WebAssembly bridge for the PKL language server. It exposes the
parser-backed `pkl-lsp-core` engine to browser-hosted editor integrations
through `wasm-bindgen`.

Build for the WebAssembly target with:

```sh
wasm-pack build crates/pkl-lsp-wasm --target web
```

The package enables only pklr's parser feature, so it does not pull evaluator,
network, or native host-I/O dependencies into the WASM dependency tree.
See [docs.rs] for the generated API documentation.

[docs.rs]: https://docs.rs/pkl-lsp-wasm
