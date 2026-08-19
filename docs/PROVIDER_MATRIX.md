# Provider matrix

One-page comparison of every provider in scope. Detail and evidence live in
[`research/`](research/); this table is the summary an implementer reads first.

Confidence markers follow [`research/README.md`](research/README.md).
Observations were made on Linux (NixOS) in August 2026 against the tool versions
listed in each research note.

| Provider        | Auth kinds                    | Credential location                                                                | Credential store                                            | Switch without re-auth                                                                         | Quota signal                                                                                       | Base-URL override                                                          | Adapter difficulty                                                              |
| --------------- | ----------------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **Codex CLI**   | OAuth (ChatGPT plan), API key | `~/.codex/auth.json` `[verified-local]`                                            | Plain JSON file `[verified-local]`                          | Likely — single self-contained file, plus `CODEX_HOME` relocation `[inferred]`                 | Not observed locally `[unknown]`                                                                   | Yes, OpenAI-compatible base URL `[verified-docs]`                          | **Low** — one document to swap, and a second env-var strategy as backup         |
| **Grok CLI**    | OAuth (OIDC), API key         | `~/.grok/auth.json` `[verified-local]`                                             | Plain JSON file, keyed per provider scope `[verified-docs]` | By relocating `$GROK_HOME` `[verified-docs]`; unprobed on 0.2.93 `[unknown]`                   | `models_cache.json` exists; contents unconfirmed `[unknown]`                                       | `GROK_CLI_CHAT_PROXY_BASE_URL` `[verified-docs]`; plan session `[unknown]` | **Low–medium** — `$GROK_HOME` per account, but advisory locks must be respected |
| **Claude Code** | OAuth, API key                | `~/.claude/.credentials.json` `[verified-local]`                                   | Plain JSON file `[verified-local]`                          | Likely, but two files must move together `[inferred]`                                          | `rateLimitTier` present in credentials; no usage counter observed `[verified-local]` / `[unknown]` | Yes, `ANTHROPIC_BASE_URL` `[verified-docs]`                                | **Medium** — identity is split between `.credentials.json` and `~/.claude.json` |
| **Gemini CLI**  | OAuth, API key                | Not observed — installation was not signed in `[unknown]`                          | Unknown `[unknown]`                                         | Yes for API-key accounts via `GEMINI_API_KEY` `[verified-docs]`; unknown for OAuth `[unknown]` | Unknown `[unknown]`                                                                                | Yes, API-key mode `[verified-docs]`                                        | **Medium** — needs a signed-in host to establish the OAuth path                 |
| **Cursor**      | Unknown `[unknown]`           | Not found in `~/.cursor/` or `~/.config/cursor/cli-config.json` `[verified-local]` | Suspected OS keyring or Electron storage `[inferred]`       | Unknown `[unknown]`                                                                            | Unknown `[unknown]`                                                                                | Unknown `[unknown]`                                                        | **High** — nothing about its credential handling is established yet             |

## What this implies for sequencing

Codex CLI and Grok CLI now list accounts from their on-disk `auth.json` files.
Grok lists signed-in OIDC identities and skips reserved scopes. Claude Code
lists the on-disk identity by reading `.credentials.json` and
`~/.claude.json`. Gemini CLI lists an account when `GEMINI_API_KEY` is set. It
does not list OAuth accounts: the adapter still reads only `GEMINI_API_KEY`,
not the OAuth files vendor source now names. No adapter implements switching.
Codex switching strategies are `[inferred]`. Grok CLI has no in-file
identity selection; `$GROK_HOME` is the documented switch `[verified-docs]`
and has not been probed against 0.2.93 on this host `[unknown]`. The
remaining order is unchanged.

1. **Codex CLI first.** It is the only provider whose entire credential state is
   one readable document. Listing that document is in place. The switch
   verification path is still the cheapest remaining end-to-end proof.
2. **Grok CLI second.** `$GROK_HOME` relocates the whole client home, so a
   switch never mutates the default home. The vendor lock protocol for
   `auth.json.lock` is specified and must be honoured on any write. Several
   keys in the file are reserved scopes beside one OIDC session, not several
   user identities.
3. **Claude Code third.** Listing the two-file identity is in place. A switch
   must still move both files together, which is the first case where
   atomicity across more than one file actually matters.
4. **Gemini CLI fourth.** Listing the API-key path is in place. This adapter
   still reads only `GEMINI_API_KEY` and does not list OAuth accounts. An
   OAuth switch remains unimplementable until a signed-in host observation
   closes the remaining `[unknown]` write-path questions.
5. **Cursor last**, and read-only until its credential handling is established.
   Writing a switch for a store you have not found is how you lock a user out.

This ordering is reflected in [`ROADMAP.md`](ROADMAP.md).

## Open questions blocking implementation

| Question                                                                                            | Blocks                               |
| --------------------------------------------------------------------------------------------------- | ------------------------------------ |
| Where does `cursor-agent login` persist its session?                                                | The entire Cursor adapter            |
| Where does Gemini CLI store OAuth credentials after sign-in?                                        | Gemini OAuth accounts                |
| Does any of the five expose a machine-readable quota or usage endpoint?                             | `FR-5`, and therefore `FR-7`         |
| Do Claude Code and Codex CLI honour a base-URL override in plan-auth mode, or only in API-key mode? | `FR-6` usefulness for those tools    |
| Does swapping `~/.codex/auth.json` invalidate a session server-side?                                | The core switching premise for Codex |
