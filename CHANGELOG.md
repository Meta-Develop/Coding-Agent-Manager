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
- Backup subsystem: timestamped snapshots over an adapter's declared config
  paths, restore that returns the tree to its captured state including files
  that were absent and Unix permission bits, and retention that never prunes a
  provider's newest backup (`NFR-4`).
- Atomic write helper that every config write goes through: temporary file,
  `fsync`, rename, and owner-only permissions at creation (`NFR-4`).
- OS-keychain credential store for macOS Keychain, Windows Credential Manager,
  and Freedesktop Secret Service, with error mapping that never forwards stored
  bytes (`FR-3`, `NFR-1`).
- Encrypted-file fallback store: Argon2id key derivation from a user passphrase
  with its parameters recorded in the file, ChaCha20-Poly1305 authenticated
  encryption, a verifier that reports a wrong passphrase separately from a
  tampered file, and a refusal to read or write a newer schema version. There is
  still no plaintext mode, behind any flag (`FR-3`, `NFR-1`).
- Read-only Codex CLI and Grok CLI adapters: each lists the accounts stored in
  that tool's `auth.json`. Switching is not implemented yet.
- Read-only Claude Code adapter: lists the on-disk identity by reading
  `.credentials.json` and `~/.claude.json`. Switching is not implemented yet.
- Read-only Gemini CLI adapter: lists an account when `GEMINI_API_KEY` is set.
  It does not list OAuth accounts: the adapter still reads only
  `GEMINI_API_KEY`, not the OAuth files vendor source now names
  (`~/.gemini/oauth_creds.json`, `~/.gemini/google_accounts.json`). Switching
  is not implemented; an OAuth switch remains unimplementable.

### Changed

- Declared minimum supported Rust version raised to 1.88, matching what the
  dependency tree already required.
- Linux CI dependency list now names `libdbus-1-dev` explicitly rather than
  relying on it arriving transitively.

### Notes

The credential stores and the backup machinery are implemented and exercised by
the M1 exit-criteria suite. Claude Code, Codex CLI, Grok CLI, and Gemini CLI
can list accounts; switching is not implemented. See `docs/ROADMAP.md`.

[Unreleased]: https://github.com/Meta-Develop/Coding-Agent-Manager/commits/main
