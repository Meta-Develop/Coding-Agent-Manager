# Coding Agent Manager

Unified account, credential, and quota manager for AI coding agents.

Switching between accounts on a coding agent is a manual, error-prone ritual:
log out, log in, hope the tool did not cache the old identity, repeat for every
other tool you use. Coding Agent Manager keeps every account for every agent in
one place and makes switching a single click, with no re-authentication.

> **Status: pre-alpha.** The application skeleton, the adapter contract, and the
> full specification are in place. No provider adapter is functional yet. See
> [`docs/ROADMAP.md`](docs/ROADMAP.md) for what lands when, and
> [`docs/PROVIDER_MATRIX.md`](docs/PROVIDER_MATRIX.md) for what is known about
> each tool today.

## Supported providers

| Provider    | Vendor    | Auth           | Adapter status | Notes                                                     |
| ----------- | --------- | -------------- | -------------- | --------------------------------------------------------- |
| Claude Code | Anthropic | OAuth, API key | Planned        | Credentials and client state live in two separate files   |
| Codex CLI   | OpenAI    | OAuth, API key | Planned        | Single self-contained `auth.json`; cleanest target        |
| Cursor      | Anysphere | Unknown        | Planned        | Credential location not yet established                   |
| Grok CLI    | xAI       | OAuth, API key | Planned        | Multi-identity `auth.json`; may switch without file swaps |
| Gemini CLI  | Google    | OAuth, API key | Planned        | `GEMINI_API_KEY` gives a file-free switching path         |

Antigravity, Windsurf, GitHub Copilot, OpenCode, Aider, and Cline are planned
behind the same adapter interface. Adding one does not require changing core
code — see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## What it does

- **Multi-account management** per provider, with one-click switching.
- **Credential handling**: OAuth 2.0 with PKCE, API keys, automatic refresh.
- **Secure storage**: OS keychain first, encrypted file only as a fallback.
- **Quota dashboard**: remaining quota, rate-limit windows, and reset times for
  the providers that expose a usable signal.
- **Local relay**: one endpoint that adapts between OpenAI, Anthropic, and
  Gemini wire formats, so any tool can talk to any account.
- **Smart routing**: model mapping and tiered routing by account type and
  remaining quota, with failover on rate limits.

The full requirement list, numbered and referenceable, is in
[`docs/SPEC.md`](docs/SPEC.md).

## Screenshots

_Placeholder — screenshots will be added when the first adapter ships (M2)._

## Quick start

```bash
git clone https://github.com/Meta-Develop/Coding-Agent-Manager.git
cd Coding-Agent-Manager
npm install
npm run tauri:dev
```

### Building on NixOS

This repository ships a dev shell, because Tauri v2 needs `webkit2gtk-4.1`,
which most NixOS hosts do not provide system-wide:

```bash
nix develop
npm install
npm run tauri:build
```

Full prerequisites for every platform are in
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Documentation

| Document                                           | What it covers                                        |
| -------------------------------------------------- | ----------------------------------------------------- |
| [`docs/SPEC.md`](docs/SPEC.md)                     | Numbered functional and non-functional requirements   |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)     | Layering, adapter contract, relay and router design   |
| [`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md) | Threat model and controls for handling credentials    |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)       | Environment setup, commands, adding an adapter        |
| [`docs/TESTING.md`](docs/TESTING.md)               | Test strategy, including testing without real secrets |
| [`docs/ROADMAP.md`](docs/ROADMAP.md)               | Milestones M0 through v1.0                            |
| [`docs/research/`](docs/research/)                 | Per-provider evidence notes with confidence markers   |
| [`docs/adr/`](docs/adr/)                           | Architecture decision records                         |

## Security

This application handles other people's credentials, so it is built to a higher
bar than a typical desktop utility: no telemetry, secrets never touch a log, the
relay binds to loopback unless you explicitly say otherwise, and no config file
is ever replaced without a recoverable backup. The reasoning is in
[`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md); to report a vulnerability,
see [`SECURITY.md`](SECURITY.md).

## Contributing

Contributions are welcome, especially provider research and new adapters. Start
with [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Relationship to other projects

Coding Agent Manager is an independent, clean-room implementation. It was
inspired by the _problem_ that [`lbjlaq/Antigravity-Manager`][upstream] solves
for a single vendor, and generalises it across every major coding agent. It
shares no code, assets, or text with that project, and is not a fork of it.
The reasoning is recorded in
[`docs/adr/0006-clean-room-independent-implementation.md`](docs/adr/0006-clean-room-independent-implementation.md).

[upstream]: https://github.com/lbjlaq/Antigravity-Manager

**This project is not affiliated with, endorsed by, or sponsored by Anthropic,
OpenAI, Anysphere, xAI, Google, or any other vendor whose tools it manages.**
All product names are trademarks of their respective owners. Using this tool to
manage your own accounts is your responsibility under each vendor's terms of
service.

## License

[GPL-3.0-or-later](LICENSE) © Meta-Develop
