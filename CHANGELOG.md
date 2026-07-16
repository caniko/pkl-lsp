# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-07-16

### Changed

- Use the crates.io `pklr` 1.2.0 parser-only feature instead of the temporary
  vendored workspace copy.
- Add crate-local metadata, documentation, and license assets for crates.io
  publication.

## [0.2.0] - 2026-07-08

- Expands parser-backed LSP capabilities for richer Pkl editor support.
- Improves native stdio server behavior and web extension integration.
- Updates bundled grammar coverage for additional Pkl syntax.

## [0.1.0]

- Initial PKL language extension for VS Code, VSCodium, and web-compatible clients.
- Adds native stdio LSP integration for desktop clients.
- Adds WASM-backed editor features for web clients.
- Adds parser-backed diagnostics, symbols, hover, completion, definition, and references.
