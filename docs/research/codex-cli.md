# Codex CLI (OpenAI)

## 1. Identity

- Tool: `codex`, distributed as `codex-cli`.
- Vendor: OpenAI.
- Version observed: **0.144.4** `[verified-local]`.
- OS observed: Linux (NixOS), August 2026.

## 2. Config locations

| Path                   | Purpose                                         | Marker             |
| ---------------------- | ----------------------------------------------- | ------------------ |
| `~/.codex/auth.json`   | Credentials                                     | `[verified-local]` |
| `~/.codex/config.toml` | Client configuration, per-project trust entries | `[verified-local]` |
| `$CODEX_HOME`          | Overrides the whole `~/.codex` directory        | `[verified-docs]`  |

macOS is expected to use the same `~/.codex` layout `[inferred]`; Windows is
expected to use `%USERPROFILE%\.codex` `[inferred]`. Both need confirmation on a
real host.

## 3. Credential format

`~/.codex/auth.json` `[verified-local]`, key names only:

```jsonc
{
  "auth_mode": "<string>", // e.g. a plan-based or api-key mode
  "OPENAI_API_KEY": null, // null while signed in through a plan
  "tokens": {
    "id_token": "<redacted>",
    "access_token": "<redacted>",
    "refresh_token": "<redacted>",
    "account_id": "<redacted>",
  },
  "last_refresh": "<timestamp string>",
}
```

The whole credential state is one flat document. That single fact is what makes
Codex the cheapest first adapter.

## 4. Authentication flow

- `codex login` performs a browser-based sign-in `[verified-docs]`.
- An API key can be supplied instead, in which case `OPENAI_API_KEY` is
  populated and `tokens` is expected to be absent or unused `[inferred]`.
- `last_refresh` suggests the CLI refreshes on its own schedule and rewrites the
  file in place `[inferred]`.

## 5. Account switching mechanics

Two candidate strategies:

1. **Swap `auth.json`.** Back up, write the target account's document
   atomically, and let the CLI pick it up on next start. Simple, and the
   likeliest approach `[inferred]`.
2. **Relocate `CODEX_HOME`.** Keep one directory per account and point the
   environment variable at the right one `[verified-docs]`. This never mutates a
   file the user's tool owns, which makes it strictly safer, but it only works
   for sessions this application launches or for shells the user configures.

Strategy 2 is the safer default where the launch path can be controlled;
strategy 1 is needed for a switch that affects an already-configured shell.

Unresolved: whether replacing `auth.json` invalidates the session server-side,
and whether the CLI caches identity anywhere outside that file `[unknown]`.

## 6. Quota and usage signals

No local quota file was observed `[verified-local]`. Rate-limit information is
expected on API responses as headers `[inferred]`. Nothing usable for a
dashboard has been confirmed `[unknown]`.

## 7. API surface and base-URL override

Codex speaks OpenAI's wire format. An OpenAI-compatible base URL can be
configured `[verified-docs]`, which makes it a natural client for the relay.
Whether an override is honoured while authenticated through a plan rather than
an API key is `[unknown]` and matters for `FR-6`.

## 8. Risks and constraints

- The CLI rewrites `auth.json` during refresh, so a switch racing a refresh
  could lose one side's write. Any writer must take the same precautions a
  concurrent process would.
- `config.toml` accumulates per-project trust entries. A switch must not discard
  them; they are not credentials and belong to the machine, not the account.

## 9. Open questions

- Does replacing `auth.json` invalidate the session server-side?
- Is there a lock file or advisory locking around `auth.json`?
- Windows and macOS paths, confirmed on real hosts.
- Are rate-limit headers exposed anywhere a manager could read them?
