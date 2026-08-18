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
| Backups of those configs                       | Contain the same secrets as the originals.                                               |

## 2. Trust boundaries

```text
[ Vendor APIs ]  <--network-->  [ Rust core ]  <--IPC-->  [ Webview ]
                                     |
                                     +--filesystem--> [ Managed tool configs ]
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

| #   | Threat                                                     | Control                                                                                                                                                                                                         |
| --- | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T1  | Local malware reads secrets from disk                      | OS credential service holds secrets; the encrypted-file fallback is passphrase-derived; no plaintext mode exists.                                                                                               |
| T2  | Secrets leak through logs, crash reports, or diagnostics   | `Secret` implements neither `Debug` nor `Serialize` and zeroes on drop. Error variants carry kinds and paths, never values. Diagnostic exports are built from an allowlist of fields, not by serialising state. |
| T3  | Secrets leak through the UI                                | Secrets never cross the IPC boundary. Identities are masked before they leave Rust.                                                                                                                             |
| T4  | Relay exposed to the network                               | Binds to `127.0.0.1` by default. A non-loopback binding is rejected unless an auth token is set — enforced in `RelayConfig::validate`, with tests. The UI states the risk in plain language before the opt-in.  |
| T5  | Malicious or compromised dependency                        | Lockfiles committed; `cargo audit` and `npm audit` in CI; dependency additions reviewed; the dependency set is kept deliberately small.                                                                         |
| T6  | Backup files leak secrets                                  | Backups live in the application data directory with `0600` permissions on Unix, are covered by retention pruning, and are never included in a diagnostic bundle.                                                |
| T7  | A switch corrupts a working config                         | Timestamped backup before any write; atomic temp-file-plus-rename; verification after write; automatic restore on failure (`NFR-4`).                                                                            |
| T8  | Token refresh races produce a revoked token                | Per-account async lock; a refresh failure marks the account as needing re-auth rather than retrying blindly.                                                                                                    |
| T9  | A user is tricked into importing someone else's credential | Import always shows exactly which file was read and which identity was found, before anything is stored.                                                                                                        |
| T10 | Telemetry or update checks leak usage patterns             | No telemetry, no analytics, no automatic update check that transmits identity (`NFR-7`).                                                                                                                        |

## 4. What this project will never do

- Store a secret in plaintext, in any mode, behind any flag.
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

## 6. Reporting

Vulnerability reporting is described in [`../SECURITY.md`](../SECURITY.md).
Anything in §4 being false is a vulnerability, not a bug report.
