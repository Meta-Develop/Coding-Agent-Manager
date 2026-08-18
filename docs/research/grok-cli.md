# Grok CLI (xAI)

## 1. Identity

- Tool: `grok`.
- Vendor: xAI.
- Version observed: **0.2.93** `[verified-local]`.
- OS observed: Linux (NixOS), August 2026.

## 2. Config locations

| Path                                                                       | Purpose                                   | Marker             |
| -------------------------------------------------------------------------- | ----------------------------------------- | ------------------ |
| `~/.grok/auth.json`                                                        | Credentials, keyed per identity           | `[verified-local]` |
| `~/.grok/auth.json.lock`                                                   | Advisory lock for the above               | `[verified-local]` |
| `~/.grok/config.toml`                                                      | Client configuration, marketplace sources | `[verified-local]` |
| `~/.grok/managed_config.lock`, `.config-init.lock`                         | Config locks                              | `[verified-local]` |
| `~/.grok/active_sessions.json` + `.lock`                                   | Running session registry                  | `[verified-local]` |
| `~/.grok/models_cache.json`                                                | Cached model list                         | `[verified-local]` |
| `~/.grok/agent_id`, `sessions/`, `logs/`, `skills/`, `bundled/`, `vendor/` | Client state                              | `[verified-local]` |

## 3. Credential format

`~/.grok/auth.json` `[verified-local]` is a **map keyed by identity**, with keys
shaped `"<oidc-issuer>::<client-uuid>"`. Key names only:

```jsonc
{
  "<issuer>::<client-uuid>": {
    "key": "<redacted>",
    "auth_mode": "<string>",
    "create_time": "<timestamp string>",
    "user_id": "<redacted>",
    "email": "<redacted>",
    "first_name": "<redacted>",
    "profile_image_asset_id": "<redacted>",
    "principal_type": "<string>",
    "principal_id": "<redacted>",
    "team_id": "<redacted>",
    "coding_data_retention_opt_out": false,
    "refresh_token": "<redacted>",
    "expires_at": "<timestamp string>",
    "oidc_issuer": "<url>",
    "oidc_client_id": "<uuid>",
  },
}
```

## 4. Authentication flow

OIDC — the entry carries `oidc_issuer` and `oidc_client_id` alongside a
`refresh_token` and an `expires_at`, so both the flow and the lifetime are
locally observable `[verified-local]`. An API-key mode is implied by `auth_mode`
`[inferred]`.

## 5. Account switching mechanics

The file is **already multi-identity**: several entries can coexist under
different keys `[verified-local]`. That strongly suggests the CLI selects an
active identity rather than requiring a file swap `[inferred]`.

If true, switching becomes the least invasive of the initial five — mark a
different entry active rather than overwriting credentials. What is not yet
known is _how_ the CLI decides which entry is active: an explicit field, a
separate pointer file, most-recently-used, or a command `[unknown]`.

Whatever the mechanism, any write must acquire `auth.json.lock` in the same way
the CLI does. The presence of `active_sessions.json` and its lock also means a
switch performed while a session is running is detectable, and should be
refused rather than silently applied.

## 6. Quota and usage signals

`models_cache.json` exists and may carry availability information; its contents
were not inspected `[unknown]`. No quota counter was observed `[verified-local]`.

## 7. API surface and base-URL override

xAI publishes an OpenAI-compatible API `[verified-docs]`. Whether the CLI
accepts a base-URL override is `[unknown]`.

## 8. Risks and constraints

- Advisory locks are real here, not decorative. Ignoring them risks corrupting
  a file that holds every identity at once — the highest-blast-radius file in
  the initial set.
- The entry contains personal fields (`email`, `first_name`,
  `profile_image_asset_id`). They must be masked before display and must never
  be written to a log or a diagnostic bundle.

## 9. Open questions

- How is the active identity selected among several entries?
- What is the exact lock protocol for `auth.json.lock`?
- Does `models_cache.json` expose anything quota-shaped?
- Windows and macOS paths, confirmed on real hosts.
