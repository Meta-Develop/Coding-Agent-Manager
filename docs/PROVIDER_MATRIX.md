# Provider matrix

One-page comparison of every provider in scope. Detail and evidence live in
[`research/`](research/); this table is the summary an implementer reads first.

Confidence markers follow [`research/README.md`](research/README.md).
Observations were made on Linux (NixOS) in August 2026 against the tool versions
listed in each research note.

| Provider        | Auth kinds                    | Credential location                                                                | Credential store                                       | Switch without re-auth                                                                         | Quota signal                                                                                       | Base-URL override                                 | Adapter difficulty                                                              |
| --------------- | ----------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------- |
| **Codex CLI**   | OAuth (ChatGPT plan), API key | `~/.codex/auth.json` `[verified-local]`                                            | Plain JSON file `[verified-local]`                     | Likely — single self-contained file, plus `CODEX_HOME` relocation `[inferred]`                 | Not observed locally `[unknown]`                                                                   | Yes, OpenAI-compatible base URL `[verified-docs]` | **Low** — one document to swap, and a second env-var strategy as backup         |
| **Grok CLI**    | OAuth (OIDC), API key         | `~/.grok/auth.json` `[verified-local]`                                             | Plain JSON file, keyed per identity `[verified-local]` | Likely — the file already holds multiple identities `[inferred]`                               | `models_cache.json` exists; contents unconfirmed `[unknown]`                                       | Unconfirmed `[unknown]`                           | **Low–medium** — multi-identity by design, but advisory locks must be respected |
| **Claude Code** | OAuth, API key                | `~/.claude/.credentials.json` `[verified-local]`                                   | Plain JSON file `[verified-local]`                     | Likely, but two files must move together `[inferred]`                                          | `rateLimitTier` present in credentials; no usage counter observed `[verified-local]` / `[unknown]` | Yes, `ANTHROPIC_BASE_URL` `[verified-docs]`       | **Medium** — identity is split between `.credentials.json` and `~/.claude.json` |
| **Gemini CLI**  | OAuth, API key                | Not observed — installation was not signed in `[unknown]`                          | Unknown `[unknown]`                                    | Yes for API-key accounts via `GEMINI_API_KEY` `[verified-docs]`; unknown for OAuth `[unknown]` | Unknown `[unknown]`                                                                                | Yes, API-key mode `[verified-docs]`               | **Medium** — needs a signed-in host to establish the OAuth path                 |
| **Cursor**      | Unknown `[unknown]`           | Not found in `~/.cursor/` or `~/.config/cursor/cli-config.json` `[verified-local]` | Suspected OS keyring or Electron storage `[inferred]`  | Unknown `[unknown]`                                                                            | Unknown `[unknown]`                                                                                | Unknown `[unknown]`                               | **High** — nothing about its credential handling is established yet             |

## What this implies for sequencing

1. **Codex CLI first.** It is the only provider whose entire credential state is
   one readable document, which makes it the cheapest way to prove the adapter
   contract, the backup subsystem, and the switch verification path end to end.
2. **Grok CLI second.** Its multi-identity file exercises the "several accounts
   coexist" case that the domain model assumes, and forces the advisory-lock
   handling that other adapters will also need.
3. **Claude Code third.** It introduces the two-file switch, which is the first
   case where atomicity across more than one file actually matters.
4. **Gemini CLI fourth**, starting with the API-key path, which is file-free and
   therefore low-risk, and deferring OAuth until a signed-in host is available.
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
