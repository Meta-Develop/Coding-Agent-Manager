# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Project foundation: Tauri v2 + React + TypeScript application skeleton that
  builds and runs, with routed pages for Dashboard, Accounts, Providers, Relay,
  Router, and Settings.
- `ProviderAdapter` contract, and stub adapters for Claude Code, Codex CLI,
  Cursor, Grok CLI, and Gemini CLI, each documenting its researched config paths
  with confidence markers.
- `CredentialStore` contract with OS-keychain and encrypted-file backends
  stubbed; no plaintext backend exists by design.
- `RelayConfig` validation refusing a non-loopback binding without an
  authentication token, with tests.
- Documentation set: specification with numbered requirements, architecture,
  security model, development and testing guides, roadmap, release process,
  glossary, provider matrix, five provider research notes, and six ADRs.
- `flake.nix` dev shell providing the Tauri v2 Linux prerequisites.
- CI across Linux, macOS, and Windows; release workflow for all platform bundles.

### Notes

No provider adapter is functional yet. See `docs/ROADMAP.md`.

[Unreleased]: https://github.com/Meta-Develop/Coding-Agent-Manager/commits/main
