# Roadmap

Milestones are sequenced by dependency, not by ambition. The ordering follows
one rule: **nothing writes to a user's config until the thing that restores it
works.**

Requirement ids refer to [`SPEC.md`](SPEC.md). Provider ordering and its
justification are in [`PROVIDER_MATRIX.md`](PROVIDER_MATRIX.md).

---

## M0 — Foundation ✅

**Goal.** A repository that builds, a specified product, and an adapter contract
worth implementing against.

- Tauri v2 + React + TypeScript skeleton that builds and runs.
- `ProviderAdapter` and `CredentialStore` contracts defined.
- Five adapter modules stubbed, each carrying its researched config paths.
- Full documentation set: spec, architecture, security model, research notes,
  ADRs.
- CI on Linux, macOS, and Windows.

**Exit criteria.** `npm run typecheck`, `npm run lint`, `cargo check`,
`cargo test`, and `cargo clippy -D warnings` all pass in CI on all three
platforms.

---

## M1 — Safe foundations ✅

**Goal.** The machinery that makes writing to someone's config defensible.

Satisfies: `FR-3`, `NFR-1`, `NFR-4`.

- Backup subsystem: timestamped snapshots over `config_paths()`, restore,
  retention with a never-prune floor.
- Atomic write helper: temp file, `fsync`, rename.
- `CredentialStore` implemented for the OS keychain on all three platforms.
- Encrypted-file fallback with passphrase derivation.
- Secret redaction proven by test, including the fixture-leak grep.

**Exit criteria.** A forced failure at any point during a simulated switch
leaves the fixture tree byte-identical to its pre-switch state. No secret
appears in any log, error, or diagnostic under test.

---

## M2 — First adapters: Codex CLI and Grok CLI

**Goal.** Prove the contract end to end on the two providers whose credential
state is a single readable document.

Satisfies: `FR-1`, `FR-2`, `FR-4`.

- Codex CLI: read, list, switch by `auth.json` replacement, plus the
  `CODEX_HOME` strategy where the launch path is controlled.
- Grok CLI: read, list signed-in OIDC identities, switch by relocating
  `$GROK_HOME` per account, honouring `auth.json.lock` and refusing while a
  session is active.
- Accounts page: import, label, list, switch, with switch verification.
- Adapter contract test suite, running over both.

**Exit criteria.** A real switch on a real machine, verified by the tool
reporting the expected identity, with a working restore. Both adapters at
`experimental`.

---

## M3 — Claude Code and Gemini CLI

**Goal.** The two-file switch, and the file-free switch.

Satisfies: `FR-1`, `FR-2` for two more providers.

- Claude Code: surgical read-modify-write of the identity fields in
  `~/.claude.json`, preserving `projects`, `mcpServers`, and machine state,
  atomic across both files.
- Gemini CLI: API-key accounts via `GEMINI_API_KEY`. OAuth stays unimplemented
  until its credential path is `[verified-local]`.

**Exit criteria.** Claude Code switch preserves every machine-scoped field,
proven by fixture diff. Gemini API-key switching works without touching a file.

---

## M4 — Quota visibility

**Goal.** Show what is actually knowable, and nothing more.

Satisfies: `FR-5`, `NFR-8`.

- Quota collection per adapter, returning empty where no signal exists.
- Dashboard with list and grid views, plan labels, reset times, error states.
- Explicit "no quota signal available" rendering — never a fabricated number.

**Exit criteria.** Every provider either shows a sourced number with its
`capturedAt` age, or states that it publishes no signal.

---

## M5 — Relay

**Goal.** One local endpoint, three dialects.

Satisfies: `FR-6`, `FR-8`, `FR-9`.

- Loopback HTTP listener with per-dialect path prefixes.
- OpenAI ⇄ Anthropic ⇄ Gemini translation, non-streaming then streaming.
- Image-generation passthrough with size and quality controls.
- Reasoning-budget mapping, with explicit errors on unmappable fields.
- Non-loopback binding gated on an auth token, enforced and tested.

**Exit criteria.** Golden-file coverage for every case in
[`TESTING.md`](TESTING.md) §5, and a real coding agent driven through the relay
against a different vendor's account.

---

## M6 — Routing

**Goal.** Spend the right account's quota.

Satisfies: `FR-7`.

- Ordered rules: model pattern → provider + upstream model.
- Quota-gated rules, using M4's signals.
- Failover on rate-limit responses, with throttle-until tracking.
- No implicit fallback: an unmatched request errors.

**Exit criteria.** A rate-limited account demonstrably fails over, and no
request is ever served by an account no rule selected.

---

## M7 — Cursor, and packaging

**Goal.** Close the initial provider set and ship installable artifacts.

Satisfies: `FR-10`, and `FR-4` for Cursor.

- Cursor: detection and read-only support. Switching only if and when its
  credential store becomes `[verified-local]`.
- Signed Windows NSIS installer, macOS `.dmg` for both architectures, Linux
  `.deb` / `.rpm` / AppImage, Docker image for the headless relay.
- Release workflow producing checksummed artifacts.

**Exit criteria.** A user can install from a release artifact on each platform
and complete a switch.

---

## v1.0

Every `FR-*` either satisfied or explicitly deferred with a recorded reason;
every `NFR-*` satisfied; adapters honestly labelled; documentation matching
behaviour.

## Beyond v1

Antigravity, Windsurf, GitHub Copilot, OpenCode, Aider, and Cline adapters.
Profiles that switch several tools at once. Optional encrypted export for moving
accounts between the user's own machines. A runtime plugin system, only if a
credible sandbox for third-party adapter code exists — see
[`adr/0002-provider-adapter-plugin-architecture.md`](adr/0002-provider-adapter-plugin-architecture.md).
