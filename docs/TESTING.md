# Testing

## 1. Strategy

| Layer                | Tool                              | What it covers                                                                     |
| -------------------- | --------------------------------- | ---------------------------------------------------------------------------------- |
| Domain and core      | `cargo test`                      | Registry integrity, error mapping, relay config validation, router rule selection. |
| Adapter contract     | `cargo test` over fixtures        | Every adapter behaves identically against a synthetic home directory.              |
| Protocol translation | Golden files                      | OpenAI ⇄ Anthropic ⇄ Gemini request and response translation, including streaming. |
| Front end            | `tsc --noEmit`, ESLint            | Type-level contract between `model.rs` and `types/index.ts`.                       |
| End to end           | Manual checklist, later automated | Switch, verify, restore on a disposable machine profile.                           |

## 2. Contract tests

Every adapter is exercised by the same test body, parameterised over the
registry. A new adapter therefore inherits the whole suite for free, and cannot
quietly skip a rule.

The contract asserts:

- `id()` is stable, kebab-case, and unique across the registry.
- `descriptor().maturity` is not `supported` unless `list_accounts()` succeeds.
- `descriptor().capabilities` matches whether `add_account`,
  `activate_account`, and `delete_account` return `NotImplemented`
  (`NFR-8`). The probe id is not a safe path component, so an
  implementation refuses before creating a directory, writing the live
  home, or spawning the vendor CLI.
- `config_paths()` returns absolute paths and never panics when `$HOME` is
  unset or unusual.
- `detect()` is side-effect free: running it twice leaves the fixture home byte
  for byte identical.
- `list_accounts()` never returns a field containing a value that appears in the
  fixture's secret material. This is the automated form of `NFR-1`.
- `activate_account()` is implemented or returns exactly
  `NotImplemented`. An unimplemented adapter writes nothing to the
  fixture home. Backup-before-write and restore-on-failure (`NFR-4`) are
  proven in the Codex CLI adapter unit tests, not yet in this shared
  body.

## 3. Fixtures

```text
src-tauri/tests/fixtures/<provider>/
  home/                 synthetic $HOME tree the adapter sees
  expected/accounts.json  what list_accounts() must return
  expected/after-switch/  the tree after a successful switch
```

Fixture rules:

- Every secret-shaped value is an obvious fake: `FAKE-access-token-0001`. The
  leak test greps for that prefix, which only works if real values never appear.
- Fixtures are copied into a `tempfile::TempDir` per test; a test never touches
  a real `$HOME`. Adapters take their root from an injected base path for this
  reason.
- Fixtures are derived from documented schemas, never from a copied personal
  config.

## 4. Never test with a real credential

- No test, script, or CI job may read the developer's real `$HOME`.
- No real token, key, cookie, or account identifier belongs in this repository,
  in any branch, at any time. Git history is forever.
- CI runs with no vendor credential configured. A test that would need one is a
  test that is testing the vendor, not this project.

## 5. Golden-file translation tests

`relay::translate` is a pure function, which makes it exactly the kind of thing
golden files test well:

```text
src-tauri/tests/golden/<from>-to-<to>/
  NNN-<case>.input.json
  NNN-<case>.expected.json
```

Cases must cover: a plain chat turn, a system prompt, multi-part content, tool
definitions and tool calls, a streaming sequence event by event, an image
request, a reasoning-budget field, and a field with no counterpart in the target
dialect — which must produce an explicit error, never a silent drop.

## 6. Coverage expectations

- Core, adapters, and translation: meaningful coverage of every branch that
  writes to disk or handles a secret. These are the paths where a bug costs a
  user their login. Codex CLI `add_account` is covered in that adapter's
  unit tests (fresh directory, vendor-login failure cleanup, no live-home
  mutation).
- UI: type checking and lint. The Accounts page is no longer a
  placeholder. Component tests are still not required.
- A pull request adding a write path without a test for its failure-and-restore
  case does not get merged.

## 7. Running

```bash
nix develop        # on NixOS
npm run typecheck
npm run lint
npm run rust:test
npm run rust:clippy
```

CI runs the same commands on Linux, macOS, and Windows — see
[`../.github/workflows/ci.yml`](../.github/workflows/ci.yml).
