# Opencode Integration Gaps

This file tracks limitations that matter for opencode's native LSP integration.
The integration is useful today for opened-document diagnostics and simple
symbol queries, but the server does not yet provide full workspace semantics.

- Workspace indexing only includes opened documents. Opencode will not receive
  whole-workspace PKL diagnostics until files are read, edited, or otherwise
  touched by the LSP client.
- Diagnostics are lex/parse oriented. Evaluator, import, package, and host IO
  diagnostics are not fully wired through pklr yet.
- Definitions and references are based on the local symbol/text index, not
  deterministic semantic resolution across imports and inheritance.
- `workspace/symbol`, `textDocument/implementation`, and call hierarchy methods
  are not implemented.
- Pull diagnostics are not advertised. Clients should rely on
  `textDocument/publishDiagnostics` after `didOpen` and `didChange`.
- Import targets are recorded in the index but not resolved to opened document
  URIs unless those documents are already present in the document store.
