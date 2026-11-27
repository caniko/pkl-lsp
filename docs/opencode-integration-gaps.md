# OpenCode integration gaps

OpenCode's native LSP integration supports diagnostics for open files and
simple symbol queries, not full workspace semantics.

- The server indexes only open documents. Workspace PKL diagnostics cover only
  documents in the server's store; the server does not scan the workspace.
- Diagnostics cover lex/parse errors. Evaluator, import, package, and host IO
  diagnostics are not fully wired through pklr.
- Definitions and references use local symbol/text matching, not deterministic
  semantic resolution across imports and inheritance. The index records import
  targets but cannot resolve targets outside the document store to document URIs.
- The native server handles `workspace/symbol`; `textDocument/implementation`
  reuses type-definition lookup. Call hierarchy is unsupported.
- The native server advertises pull diagnostics and still sends
  `textDocument/publishDiagnostics` after `didOpen` and `didChange`.
