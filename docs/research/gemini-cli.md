# Gemini CLI (Google)

## 1. Identity

- Tool: `gemini`.
- Vendor: Google.
- Version observed: **0.47.0** `[verified-local]`.
- OS observed: Linux (NixOS), August 2026.

## 2. Config locations

| Path                                     | Purpose                                      | Marker             |
| ---------------------------------------- | -------------------------------------------- | ------------------ |
| `~/.gemini/projects.json`                | Project registry; empty on the observed host | `[verified-local]` |
| `~/.gemini/settings.json`                | User settings                                | `[inferred]`       |
| OAuth credential file under `~/.gemini/` | Credentials                                  | `[unknown]`        |

The inspected installation had **not** completed a sign-in, so no credential
file existed to observe. This is the single largest gap in the initial research
set, and it blocks the Gemini OAuth path entirely.

## 3. Credential format

`[unknown]`. Must be established on a signed-in host before any write path is
implemented.

## 4. Authentication flow

Two documented modes `[verified-docs]`:

1. **OAuth sign-in** through a Google account, in a browser.
2. **API key** supplied through the `GEMINI_API_KEY` environment variable.

## 5. Account switching mechanics

- **API-key accounts**: switching is purely environmental — set `GEMINI_API_KEY`
  for the launched process `[verified-docs]`. No file is touched, so there is no
  corruption risk and no backup needed. This is the safest switch in the entire
  initial set and should be implemented first for this provider.
- **OAuth accounts**: `[unknown]` until the credential path is established.

## 6. Quota and usage signals

`[unknown]`. Free-tier limits are documented as request-rate limits
`[verified-docs]`, but no local signal was observed.

## 7. API surface and base-URL override

Gemini `generateContent` format `[verified-docs]`. Google also publishes an
OpenAI-compatible endpoint `[verified-docs]`, which gives the relay two possible
integration shapes for the same vendor.

## 8. Risks and constraints

- Building an adapter against `[unknown]` credential handling is exactly the
  failure mode this documentation set exists to prevent. Ship the API-key path,
  and leave OAuth unimplemented until evidence exists.

## 9. Open questions

- Where are OAuth credentials persisted after `gemini` sign-in?
- Is there a settings file, and does it carry account identity?
- Does the CLI support multiple concurrent accounts natively?
- Windows and macOS paths, confirmed on real hosts.
