# Security model

This application exists to hold, move, and write other people's credentials. A
bug here does not produce a broken feature; it produces a leaked account. This
document is therefore a design constraint, not a formality.

## 1. Assets

| Asset                                          | Why it matters                                                                           |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------- |
| OAuth access and refresh tokens                | Direct account access. A refresh token is long-lived and often not revocable per-device. |
| API keys                                       | Direct account access, usually billable.                                                 |
| Account identities (emails, org ids, user ids) | Personally identifying; useful for targeted phishing.                                    |
| The user's existing tool configs               | Destroying one locks the user out of a working tool.                                     |
| Stored per-account `auth.json` copies          | Same secrets as the live tool home, in a directory this application created.             |
| Backups of those configs                       | Contain the same secrets as the originals.                                               |

## 2. Trust boundaries

```text
[ Vendor APIs ]  <--network-->  [ Rust core ]  <--IPC-->  [ Webview ]
                                     |
                                     +--filesystem--> [ Managed tool configs ]
                                     +--filesystem--> [ Stored account directories ]
                                     +--platform-->   [ OS credential service ]
```

- **Webview is untrusted with secrets.** It renders masked identities and opaque
  account ids. A cross-site-scripting bug in the UI must not be able to read a
  token, because the token was never sent there.
- **Adapters are trusted code, compiled in.** This is exactly why v1 has no
  runtime plugin system: a third-party adapter would run with access to every
  credential the application holds.
- **The relay is a network boundary inside the machine.** Anything listening on
  a socket is reachable by every process on the host, and by the network if
  bound wrongly.

## 3. Threats and controls

| #   | Threat                                                     | Control                                                                                                                                                                                                                                                                                                                                                                                      |
| --- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T1  | Local malware reads secrets from disk                      | Secrets this application itself stores go to the OS credential service or the encrypted-file fallback; those backends have no plaintext mode. Stored Codex accounts are vendor-written files under the application data directory, protected by filesystem permissions — a documented deviation from `FR-3`, see [ADR 0008](adr/0008-vendor-written-auth-json-for-stored-codex-accounts.md). |
| T2  | Secrets leak through logs, crash reports, or diagnostics   | `Secret` implements neither `Debug` nor `Serialize` and zeroes on drop. Error variants carry kinds and paths, never values. Diagnostic exports are built from an allowlist of fields, not by serialising state.                                                                                                                                                                              |
| T3  | Secrets leak through the UI                                | Secrets never cross the IPC boundary. Identities are masked before they leave Rust.                                                                                                                                                                                                                                                                                                          |
| T4  | Relay exposed to the network                               | Binds to `127.0.0.1` by default. A non-loopback binding is rejected unless an auth token is set — enforced in `RelayConfig::validate`, with tests. The UI states the risk in plain language before the opt-in.                                                                                                                                                                               |
| T5  | Malicious or compromised dependency                        | Lockfiles committed; `cargo audit` and `npm audit` in CI; dependency additions reviewed; the dependency set is kept deliberately small.                                                                                                                                                                                                                                                      |
| T6  | Backup files leak secrets                                  | Backups live in the application data directory with `0600` permissions on Unix, are covered by retention pruning, and are never included in a diagnostic bundle. A backup taken before a Codex switch includes the live `auth.json` and is therefore credential material.                                                                                                                    |
| T7  | A switch corrupts a working config                         | Timestamped backup before any write; atomic temp-file-plus-rename; verification after write; automatic restore on failure (`NFR-4`).                                                                                                                                                                                                                                                         |
| T8  | Token refresh races produce a revoked token                | Per-account async lock; a refresh failure marks the account as needing re-auth rather than retrying blindly.                                                                                                                                                                                                                                                                                 |
| T9  | A user is tricked into importing someone else's credential | Import always shows exactly which file was read and which identity was found, before anything is stored.                                                                                                                                                                                                                                                                                     |
| T10 | Telemetry or update checks leak usage patterns             | No telemetry, no analytics, no automatic update check that transmits identity (`NFR-7`).                                                                                                                                                                                                                                                                                                     |
| T11 | Local malware reads a stored Codex account directory       | Directories live at `{data_dir}/accounts/codex-cli/{account_id}`, created `0700` on Unix, not next to `~/.codex`. The vendor CLI writes `auth.json`; this application does not set or check that file's mode. Weaker than the OS credential service — see [ADR 0008](adr/0008-vendor-written-auth-json-for-stored-codex-accounts.md).                                                        |
| T12 | This application captures vendor login output              | `add_account` spawns `codex login` with inherited stdio. The child's stdout and stderr go to the terminal that launched this application and are never captured, logged, or copied into an error (`NFR-1`).                                                                                                                                                                                  |

## 4. What this project will never do

- Store a secret in plaintext, in any mode, behind any flag. The one
  documented exception is a stored Codex account: the vendor CLI writes
  `auth.json` into a directory this application created, and this
  application does not encrypt that file. See
  [ADR 0008](adr/0008-vendor-written-auth-json-for-stored-codex-accounts.md).
- Send a credential anywhere except to the vendor it belongs to, or to the
  managed tool's own config on the local machine.
- Collect telemetry, analytics, or usage statistics.
- Bind the relay to a non-loopback interface without an explicit opt-in and an
  authentication token.
- Include secret material in a log, an error message, a crash report, or a
  diagnostic export.
- Modify a managed tool's config without a restorable backup.
- Ship a feature whose purpose is to circumvent a vendor's rate limits, terms of
  service, or pricing.

## 5. Backup and restore

- A backup is taken before the first write of any switch, covering every path in
  `config_paths()`.
- Backups are timestamped and immutable once written.
- Retention is user-configurable with a floor: the most recent backup per
  provider is never pruned automatically.
- Restore is offered explicitly in the UI, including for configs the application
  did not itself break.
- For Codex CLI, that snapshot includes the live `auth.json`. The backup is
  another copy of the credential, stored under the application data directory
  with `0600` on Unix, the same as any other captured secret file.

## 6. Stored Codex accounts

A stored Codex account is a directory this application created, not a
secret it wrote into the credential store. Layout:

```text
{data_dir}/accounts/codex-cli/{account_id}/auth.json
```

`{data_dir}` is the application data directory from
`paths::project_dirs()`. The live tool home (`~/.codex`, or `$CODEX_HOME`
when that is set) is not one of these directories.

On Unix the account directory is created `0700`. The `auth.json` inside it
is written by `codex login`, so its mode is whatever that CLI creates.
This application does not chmod the file and does not encrypt it. See
[ADR 0008](adr/0008-vendor-written-auth-json-for-stored-codex-accounts.md).

`add_account` must not mutate the live tool home. It creates the directory,
spawns `codex login` with `CODEX_HOME` pointing at it, and inherits stdio
so the CLI's prompts and URL appear on the launching terminal. This
application never captures that output. A failed attempt removes the
directory it created and leaves sibling accounts alone.

`delete_account` removes only that directory. It does not touch the live
home, so deleting the account that is currently active does not sign the
tool out.

## 7. Reporting

Vulnerability reporting is described in [`../SECURITY.md`](../SECURITY.md).
Anything in §4 being false is a vulnerability, not a bug report.
