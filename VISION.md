# pkl-lsp vision

`pkl-lsp` is a Rust language server for [Pkl](https://pkl-lang.org/), shipped
both as a native stdio server and as a WASM bridge for browser-based editors,
plus the VS Code/Codium and IntelliJ extension sources around it. It exists so
Pkl files get diagnostics, symbols, imports, hover, completion and navigation
inside editors that speak LSP, including editors where no JVM-based server is
practical.

## Problem

Pkl is a configuration language with its own evaluator and tooling, and Canix
treats Pkl as the source of truth for fleet data. Editing Pkl without editor
feedback means finding broken syntax, stale imports and wrong types by running
a build. The available language tooling is tied to the JVM and to one editor
family, which does not cover OpenCode, VS Code web, or an IntelliJ plugin built
from the same workspace. `pkl-lsp` is a single Rust core serving all of them.

## Principles

- **Parser-only, deliberately.** The core uses `pklr`'s parser-only feature, so
  the native and WASM crates carry no evaluator or host-IO dependencies. This is
  what makes a browser target possible at all.
- **One core, several transports.** Diagnostics, symbols, imports, hover,
  completion, definition, references and archived indexes live in
  `pkl-lsp-core`; the stdio server and the WASM bridge are thin front ends.
- **Gaps are documented, not hidden.** `docs/opencode-integration-gaps.md`
  lists exactly what does not work yet: workspace indexing limited to opened
  documents, no evaluator/import/package diagnostics, text-index rather than
  semantic resolution for definitions and references, missing
  `workspace/symbol`, `textDocument/implementation` and call hierarchy, no pull
  diagnostics, and import targets not resolved to opened URIs.
- **Follow the published parser.** The workspace moved off a vendored `pklr`
  copy onto the crates.io release and adapts to its AST, diagnostics and
  property names as they change.
- **Publishing is a first-class gate.** Crates carry their own metadata,
  documentation and license assets, and the workspace is checked with
  `cargo test/clippy/fmt --all-features` style gates before release.

## Scope

In scope: parser-backed diagnostics and index features for Pkl; a native stdio
LSP server; a WASM bridge for web editors; VS Code/Codium extension sources
including Open VSX publication and signing; a JetBrains plugin built in the same
pipeline; and the documentation of remaining integration gaps.

## Non-goals

- Not an evaluator or package manager. Evaluation and host IO are outside the
  parser-only design.
- Not a full semantic language model yet. Cross-import and inheritance
  resolution, workspace-wide indexing and call hierarchy are explicitly listed
  as absent.

## Direction

Recent work is release and distribution plumbing around a stable core: adopting
`pklr` 5.2.0's published AST and property names, moving CI from Crow to GitHub
Actions with Simit and harbor-rs, building darwin servers and the JetBrains
plugin in a hosted prebuild, keeping the Gradle dependency lock current, and
verifying exact Open VSX versions and signing. The documented open thread is
closing the OpenCode integration gaps listed above.

## Place in the fleet

Canix ships composable OpenCode LSP shells whose `pkl` profile supplies this
server, so agents editing Canix's Pkl data get diagnostics inside OpenCode. The
repository is also a Simit updater target and is included in atlas maintenance
sweeps.
