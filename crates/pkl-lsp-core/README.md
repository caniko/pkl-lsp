# pkl-lsp-core

Parser-backed PKL language analysis for Language Server Protocol clients.
The crate provides document storage, workspace indexing, diagnostics, symbol
queries, completion, hover, definitions, references, and archived indexes.

Add it to a Rust project with:

```toml
[dependencies]
pkl-lsp-core = "0.2"
```

The crate uses pklr's parser-only feature and does not enable evaluator or host
I/O support. API documentation is available on [docs.rs].

[docs.rs]: https://docs.rs/pkl-lsp-core
