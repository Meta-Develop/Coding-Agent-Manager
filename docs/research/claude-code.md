# Claude Code (Anthropic)

## 1. Identity

- Tool: `claude`, distributed as Claude Code.
- Vendor: Anthropic.
- Version observed: **2.1.212** `[verified-local]`.
- OS observed: Linux (NixOS), August 2026.

## 2. Config locations

| Path                                                                                     | Purpose                          | Marker             |
| ---------------------------------------------------------------------------------------- | -------------------------------- | ------------------ |
| `~/.claude/.credentials.json`                                                            | OAuth credentials                | `[verified-local]` |
| `~/.claude.json`                                                                         | Global client state and identity | `[verified-local]` |
| `~/.claude/settings.json`                                                                | User settings                    | `[verified-local]` |
| `~/.claude/projects/`, `sessions/`, `history.jsonl`, `shell-snapshots/`, `file-history/` | Session and history data         | `[verified-local]` |
| `~/.claude/plugins/`, `cache/`, `telemetry/`, `ide/`                                     | Client-managed state             | `[verified-local]` |

Session and history data belong to the machine and the user, not to an account.
A switch must leave them alone.

## 3. Credential format

`~/.claude/.credentials.json` `[verified-local]`, key names only:

```jsonc
{
  "claudeAiOauth": {
    "accessToken": "<redacted>",
    "refreshToken": "<redacted>",
    "expiresAt": 0, // epoch milliseconds
    "refreshTokenExpiresAt": 0,
    "scopes": ["<string>"],
    "subscriptionType": "<string>",
    "rateLimitTier": "<string>",
  },
  "organizationUuid": "<redacted>",
}
```

`~/.claude.json` is a large document whose top level includes `oauthAccount`,
`userID`, `machineID`, `organizationUuid`-adjacent fields, `mcpServers`,
`projects`, and many caches and onboarding flags `[verified-local]`.

## 4. Authentication flow

- `claude` performs a browser sign-in producing the OAuth material above
  `[verified-docs]`.
- `expiresAt` and `refreshTokenExpiresAt` are both present, so both lifetimes
  are locally observable — useful for showing expiry state without a network
  call (`FR-2`).
- `subscriptionType` and `rateLimitTier` are present locally, which gives the
  dashboard a plan label even where no usage counter exists `[verified-local]`.

## 5. Account switching mechanics

Identity is **split across two files**: the tokens in `.credentials.json` and
the account/session identity in `~/.claude.json`. A switch that moves only the
first is likely to leave the client believing it is still the previous account
`[inferred]`.

A correct switch therefore has to:

1. Back up both files.
2. Replace `.credentials.json` atomically.
3. Update the identity-bearing fields inside `~/.claude.json` while preserving
   everything machine-scoped in it — `projects`, `mcpServers`, `machineID`,
   caches, onboarding flags.

That "surgical edit of a large, client-owned document" is the reason Claude Code
is a medium-difficulty adapter rather than a low one. The client rewrites
`~/.claude.json` frequently, so the edit must be a read-modify-write that
tolerates concurrent rewrites, not a whole-file replacement.

`ANTHROPIC_API_KEY` provides a separate API-key path that bypasses the OAuth
files entirely `[verified-docs]`.

## 6. Quota and usage signals

`rateLimitTier` names the tier but is not a counter `[verified-local]`. Usage
utilisation appears in client-side caches under keys such as
`cachedUsageUtilization` in `~/.claude.json` `[verified-local]`, but whether
that is stable, documented, or safe to depend on is `[unknown]`.

## 7. API surface and base-URL override

Anthropic Messages format. `ANTHROPIC_BASE_URL` redirects the client
`[verified-docs]`, which makes Claude Code a viable relay client. Whether the
override is honoured under plan authentication rather than an API key is
`[unknown]`.

## 8. Risks and constraints

- `~/.claude.json` is large, frequently rewritten, and undocumented. Editing it
  is the highest-risk write in the initial adapter set.
- The file mixes account identity with machine state, so a naive whole-file swap
  would move a user's project list and MCP servers between accounts.

## 9. Open questions

- Exactly which fields in `~/.claude.json` carry account identity?
- Does the client hold the file open or lock it?
- Are `cachedUsageUtilization` and related keys a dependable quota source?
- Windows and macOS paths, confirmed on real hosts.
