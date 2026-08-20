//! Claude Code (Anthropic) adapter.
//!
//! Observed layout on Linux, Claude Code 2.1.212:
//!
//! - `~/.claude/.credentials.json` [verified-local] — OAuth material under a
//!   `claudeAiOauth` object with the key names `accessToken`, `refreshToken`,
//!   `expiresAt`, `refreshTokenExpiresAt`, `scopes`, `subscriptionType`,
//!   `rateLimitTier`, plus a sibling `organizationUuid`.
//! - `~/.claude.json` [verified-local] — global client state: `oauthAccount`,
//!   `mcpServers`, `projects`, caches, and onboarding flags. Large and rewritten
//!   frequently by the tool.
//! - `~/.claude/settings.json` [verified-local] — user settings.
//! - `~/.claude/` also holds `projects/`, `history.jsonl`, `sessions/`,
//!   `shell-snapshots/`, `plugins/`, and caches [verified-local]. These are
//!   session data, not credentials, and must not be moved by a switch.
//!
//! Identity is split across those first two files (`docs/research/claude-code.md`
//! §5). This adapter is read-only: `list_accounts` inspects both, and
//! `activate_account` stays `NotImplemented` because a correct switch must move
//! them together and the mechanism is `[inferred]`. A write path must not
//! depend on that.
//!
//! Which file supplies each `Account` field:
//!
//! - Existence of an account — `.credentials.json`. A missing credentials file
//!   is "no account configured", even when `~/.claude.json` is present (leftover
//!   client state is not a login).
//! - `auth_kind` — presence of a `claudeAiOauth` *object* in `.credentials.json`
//!   [verified-local]. `ANTHROPIC_API_KEY` is an env-only path [verified-docs]
//!   and is never read (NFR-1).
//! - `expires_at` — `claudeAiOauth.expiresAt` when it is a number, recorded as
//!   epoch milliseconds [verified-local]. `refreshTokenExpiresAt` is the refresh
//!   lifetime, not the access expiry, and is left alone. `rateLimitTier` is a
//!   tier name, not an expiry (NFR-8).
//! - `masked_identity` — `organizationUuid` from `.credentials.json` when it is
//!   a string [verified-local], after [`mask_identity`]. It is the only
//!   identity-shaped field whose key and location are in the recorded schema.
//! - Nothing is taken from `~/.claude.json`. `oauthAccount` internals and
//!   whether `userID` is the display identity are an open question (research
//!   §9). `machineID` is machine-scoped. Listing therefore tolerates a
//!   present-but-unparsable identity file the same way it tolerates a missing
//!   one: the file is unused, so damage to it cannot hide a valid login. An
//!   unparsable `.credentials.json` is still `Error::ConfigRead` — that file
//!   is the account, and guessing at its shape is forbidden.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

/// Application-assigned id for the single on-disk Claude Code identity.
///
/// Claude Code stores one login split across two files [verified-local], so
/// this names that slot rather than echoing `organizationUuid` or anything
/// inside `oauthAccount`. SPEC §4 assigns `Account.id` here so a vendor
/// changing its identifier scheme cannot orphan local state. The same fixture
/// therefore always produces this id.
const ON_DISK_ACCOUNT_ID: &str = "claude-code-on-disk";

#[derive(Debug, Default)]
pub struct ClaudeCodeAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
}

impl ClaudeCodeAdapter {
    /// Root this adapter at `home` instead of the real user home.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
        }
    }

    fn resolved_home(&self) -> Option<PathBuf> {
        self.home.clone().or_else(home_dir)
    }

    fn config_read(&self, reason: impl Into<String>) -> Error {
        Error::ConfigRead {
            provider: self.id().to_string(),
            reason: reason.into(),
        }
    }

    /// Read a JSON object if the file exists.
    ///
    /// Missing → `Ok(None)`. Present but unreadable, not JSON, or not an
    /// object → `Error::ConfigRead`. Reason strings name the path and the
    /// kind of failure only. The serde error text can echo a token from the
    /// file; never include it (NFR-1).
    fn read_optional_object(&self, path: &Path) -> Result<Option<Map<String, Value>>> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(self.config_read(format!("{} ({})", path.display(), error.kind())));
            }
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| self.config_read(format!("{} is not valid JSON", path.display())))?;
        match value {
            Value::Object(object) => Ok(Some(object)),
            _ => Err(self.config_read(format!("{} is not a JSON object", path.display()))),
        }
    }
}

/// Mask a vendor identifier for display.
///
/// Keeps the last four characters and replaces the rest with a fixed `****`
/// prefix (e.g. `****ab12`). Returns `None` when the value is absent-as-empty
/// or too short to mask without leaving the original essentially intact.
///
/// The only identity-shaped field this adapter will pass in is
/// `organizationUuid` from `.credentials.json` [verified-local]. Callers must
/// never pass a token, a key, or a field whose shape is `[unknown]`.
fn mask_identity(raw: &str) -> Option<String> {
    const VISIBLE: usize = 4;
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= VISIBLE {
        return None;
    }
    let tail: String = chars[chars.len() - VISIBLE..].iter().collect();
    Some(format!("****{tail}"))
}

/// Convert epoch milliseconds to RFC 3339. Returns `None` on overflow or
/// if the instant cannot be formatted; the caller then leaves `expires_at`
/// unset rather than inventing a timestamp (NFR-8).
fn rfc3339_from_epoch_millis(millis: i64) -> Option<String> {
    let seconds = millis.div_euclid(1_000);
    let nanos = u32::try_from(millis.rem_euclid(1_000).saturating_mul(1_000_000)).ok()?;
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .replace_nanosecond(nanos)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// Build the single `Account` that a parsed `.credentials.json` represents.
///
/// The on-disk login is one slot [verified-local], so whatever is in the
/// credentials file is what the tool will use — `is_active` is therefore
/// always `true`.
///
/// `_identity` is the parsed `~/.claude.json` when that file exists. No
/// field is copied out of it: which of `oauthAccount`, `userID`, or the
/// `organizationUuid`-adjacent keys is the display identity is `[unknown]`
/// (research §9). The parameter is accepted so the two-file split stays
/// visible at the call site.
///
/// Classification inspects structure only (NFR-1 / threat T2):
/// - `OAuth` when `claudeAiOauth` is an object [verified-local]
/// - otherwise `AuthKind::Unknown`
///
/// Token values, `subscriptionType`, and `rateLimitTier` are never copied
/// onto the `Account`. `rateLimitTier` is quota-shaped and is not an expiry
/// or a utilisation number; `quota()` stays empty (NFR-8).
fn account_from_documents(
    provider_id: &str,
    credentials: &Map<String, Value>,
    _identity: Option<&Map<String, Value>>,
) -> Account {
    let oauth = credentials.get("claudeAiOauth").and_then(Value::as_object);
    let auth_kind = if oauth.is_some() {
        AuthKind::OAuth
    } else {
        AuthKind::Unknown
    };
    let expires_at = oauth
        .and_then(|oauth| oauth.get("expiresAt"))
        .and_then(Value::as_i64)
        .and_then(rfc3339_from_epoch_millis);
    let masked_identity = credentials
        .get("organizationUuid")
        .and_then(Value::as_str)
        .and_then(mask_identity);

    Account {
        id: ON_DISK_ACCOUNT_ID.to_string(),
        provider_id: provider_id.to_string(),
        label: "Claude Code".to_string(),
        masked_identity,
        auth_kind,
        is_active: true,
        is_selected_for_launch: false,
        // The on-disk identity lives in the tool's own home. This adapter
        // stores nothing, so mutating operations have nothing to act on.
        is_stored: false,
        is_incomplete: false,
        expires_at,
    }
}

impl ProviderAdapter for ClaudeCodeAdapter {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Claude Code".to_string(),
            vendor: "Anthropic".to_string(),
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            // Read-only listing works; switching is still NotImplemented.
            // `Supported` would overstate that (NFR-8).
            maturity: Maturity::Experimental,
            install_state: self.detect(),
            capabilities: Vec::new(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(home) = self.resolved_home() else {
            return Vec::new();
        };
        let claude_dir = home.join(".claude");
        vec![
            home.join(".claude.json"),
            claude_dir.join(".credentials.json"),
            claude_dir.join("settings.json"),
        ]
    }

    fn detect(&self) -> InstallState {
        let has_config = self
            .resolved_home()
            .map(|home| home.join(".claude").is_dir())
            .unwrap_or(false);
        if binary_on_path("claude") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        let Some(home) = self.resolved_home() else {
            return Ok(Vec::new());
        };
        let credentials_path = home.join(".claude").join(".credentials.json");
        let identity_path = home.join(".claude.json");

        // `.credentials.json` is the account. Missing → no account.
        // Present-and-unparsable → `ConfigRead`; guessing at that document
        // is forbidden.
        let credentials = self.read_optional_object(&credentials_path)?;
        // `~/.claude.json` is large, machine-scoped, and rewritten by the
        // running client. This adapter copies no field from it (research §9:
        // which key is the display identity is `[unknown]`). A truncated or
        // mid-write copy of a file we do not read must not hide a valid login,
        // so `ConfigRead` here is treated as unused — the same as missing.
        // The two-file split stays visible at the call site; only the unused
        // file is tolerated.
        let identity = match self.read_optional_object(&identity_path) {
            Ok(value) => value,
            Err(Error::ConfigRead { .. }) => None,
            Err(error) => return Err(error),
        };

        let Some(credentials) = credentials else {
            return Ok(Vec::new());
        };
        if let Some(oauth) = credentials.get("claudeAiOauth") {
            if !oauth.is_object() {
                return Err(self.config_read(format!(
                    "{} claudeAiOauth is not an object",
                    credentials_path.display()
                )));
            }
        }
        Ok(vec![account_from_documents(
            self.id(),
            &credentials,
            identity.as_ref(),
        )])
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("claude-code::activate_account"))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;
    use std::fs;
    use std::path::Path;

    const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claude-code");
    const FAKE_PREFIX: &str = "FAKE-";

    fn staged_home(name: &str) -> tempfile::TempDir {
        let src = Path::new(FIXTURE_ROOT).join(name);
        let temp = tempfile::tempdir().expect("tempdir");
        copy_tree(&src, temp.path());
        temp
    }

    fn copy_tree(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap_or_else(|error| {
            panic!("create {}: {error}", dst.display());
        });
        for entry in fs::read_dir(src).unwrap_or_else(|error| {
            panic!("read_dir {}: {error}", src.display());
        }) {
            let entry = entry.expect("dirent");
            let to = dst.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_tree(&entry.path(), &to);
            } else {
                fs::copy(entry.path(), &to).unwrap_or_else(|error| {
                    panic!(
                        "copy {} -> {}: {error}",
                        entry.path().display(),
                        to.display()
                    );
                });
            }
        }
    }

    fn assert_no_fake(where_: &str, text: &str) {
        assert!(
            !text.contains(FAKE_PREFIX),
            "{where_} leaked fixture secret material: {text}"
        );
    }

    fn write_credentials(home: &Path, body: &str) {
        let dir = home.join(".claude");
        fs::create_dir_all(&dir).expect("mkdir .claude");
        fs::write(dir.join(".credentials.json"), body).expect("write .credentials.json");
    }

    fn write_identity(home: &Path, body: &str) {
        fs::write(home.join(".claude.json"), body).expect("write .claude.json");
    }

    #[test]
    fn with_home_resolves_config_paths_under_the_injected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = ClaudeCodeAdapter::with_home(dir.path());
        let paths = adapter.config_paths();

        assert!(
            !paths.is_empty(),
            "config_paths must not go silent under an injected home"
        );
        for path in paths {
            assert!(
                path.starts_with(dir.path()),
                "{path} escaped the injected home {home}",
                path = path.display(),
                home = dir.path().display()
            );
        }
    }

    #[test]
    fn mask_identity_keeps_only_the_last_four_characters() {
        assert_eq!(mask_identity("ab12cd34"), Some("****cd34".to_string()));
        assert_eq!(mask_identity("wxyz"), None);
        assert_eq!(mask_identity("abc"), None);
        assert_eq!(mask_identity(""), None);
    }

    #[test]
    fn rfc3339_from_epoch_millis_formats_a_known_instant() {
        assert_eq!(
            rfc3339_from_epoch_millis(1_893_456_000_000).as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
        assert_eq!(rfc3339_from_epoch_millis(i64::MAX), None);
    }

    #[test]
    fn list_accounts_matches_the_full_fixture_expectation() {
        let home = staged_home("home");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let expected_path = Path::new(FIXTURE_ROOT).join("expected/accounts.json");
        let mut expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&expected_path).expect("expected/accounts.json"),
        )
        .expect("expected json");
        expected[0]["isSelectedForLaunch"] = serde_json::Value::Bool(false);
        let got = serde_json::to_value(&accounts).expect("serialize");
        assert_eq!(got, expected);

        assert_eq!(accounts.len(), 1);
        assert!(accounts[0].is_active);
        assert_eq!(accounts[0].auth_kind, AuthKind::OAuth);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        assert_eq!(
            accounts[0].expires_at.as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
        assert_eq!(accounts[0].id, ON_DISK_ACCOUNT_ID);

        let again = adapter.list_accounts().expect("second list");
        assert_eq!(again[0].id, accounts[0].id);
    }

    #[test]
    fn list_accounts_credentials_only_still_returns_the_account() {
        let home = staged_home("credentials-only");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        // Facts come from `.credentials.json`. A missing identity file is
        // not an error and does not drop the login.
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, ON_DISK_ACCOUNT_ID);
        assert_eq!(accounts[0].auth_kind, AuthKind::OAuth);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        assert!(accounts[0].is_active);
        assert_no_fake(
            "credentials-only list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
    }

    #[test]
    fn list_accounts_identity_only_returns_empty() {
        let home = staged_home("identity-only");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter
            .list_accounts()
            .expect("leftover ~/.claude.json is not an account");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_returns_empty_when_both_files_are_missing() {
        let home = staged_home("missing-files");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter
            .list_accounts()
            .expect("missing files are not an error");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_rejects_malformed_credentials() {
        // Written here rather than stored as a `.json` fixture: Prettier parses
        // tracked `*.json` and refuses this intentionally invalid document.
        let home = tempfile::tempdir().expect("tempdir");
        write_credentials(
            home.path(),
            r#"{this is not json, "accessToken": "FAKE-malformed-token-0001""#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let error = adapter
            .list_accounts()
            .expect_err("unparsable .credentials.json must not be guessed at");
        assert!(
            matches!(error, Error::ConfigRead { .. }),
            "expected ConfigRead, got {error:?}"
        );
        assert_no_fake("ConfigRead Display", &error.to_string());
        assert_no_fake("ConfigRead Debug", &format!("{error:?}"));
    }

    #[test]
    fn list_accounts_rejects_malformed_credentials_even_with_identity() {
        // The unused identity file being intact does not license guessing at
        // the credentials document. Malformed bytes written here, not stored
        // as a `.json` fixture (Prettier).
        let home = staged_home("unparsable-credentials");
        write_credentials(
            home.path(),
            r#"{this is not json, "accessToken": "FAKE-malformed-token-0001""#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let error = adapter
            .list_accounts()
            .expect_err("unparsable .credentials.json must not be guessed at");
        assert!(
            matches!(error, Error::ConfigRead { .. }),
            "expected ConfigRead, got {error:?}"
        );
        assert_no_fake("credentials ConfigRead Display", &error.to_string());
        assert_no_fake("credentials ConfigRead Debug", &format!("{error:?}"));
    }

    #[test]
    fn list_accounts_lists_when_identity_file_is_unparsable() {
        // Malformed bytes are written here, not stored as a `.json` fixture:
        // Prettier parses tracked `*.json` and refuses this document.
        let home = staged_home("unparsable-identity");
        write_identity(
            home.path(),
            r#"{this is not json, "userID": "FAKE-user-0001""#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter
            .list_accounts()
            .expect("unparsable ~/.claude.json must not hide a valid login");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, ON_DISK_ACCOUNT_ID);
        assert_eq!(accounts[0].auth_kind, AuthKind::OAuth);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        assert!(accounts[0].is_active);
        assert_no_fake(
            "unparsable-identity list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
    }

    #[test]
    fn list_accounts_unparsable_identity_only_returns_empty() {
        // Leftover client state is not a login, even when that leftover is
        // damaged. Written here rather than stored as a `.json` fixture.
        let home = staged_home("missing-files");
        write_identity(
            home.path(),
            r#"{this is not json, "userID": "FAKE-user-0001""#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter
            .list_accounts()
            .expect("damaged leftover ~/.claude.json is still not an account");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_rejects_non_object_claude_ai_oauth() {
        let home = tempfile::tempdir().expect("tempdir");
        write_credentials(
            home.path(),
            r#"{"claudeAiOauth":"FAKE-not-an-object","organizationUuid":"FAKE-organization-uuid-0001"}"#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let error = adapter
            .list_accounts()
            .expect_err("claudeAiOauth must be an object when present");
        assert!(
            matches!(error, Error::ConfigRead { .. }),
            "expected ConfigRead, got {error:?}"
        );
        assert_no_fake("non-object oauth ConfigRead", &format!("{error} {error:?}"));
    }

    #[test]
    fn list_accounts_never_reads_oauth_account_internals() {
        // `oauthAccount` internals are [unknown] (research §9). A nested
        // email in the identity file must not become `masked_identity`.
        let home = tempfile::tempdir().expect("tempdir");
        write_credentials(
            home.path(),
            r#"{"claudeAiOauth":{"expiresAt":1893456000000},"organizationUuid":"FAKE-organization-uuid-0001"}"#,
        );
        write_identity(
            home.path(),
            r#"{"oauthAccount":{"email":"FAKE-user-0001@example.invalid"},"userID":"FAKE-user-0001"}"#,
        );
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        let json = serde_json::to_string(&accounts).expect("json");
        assert!(
            !json.contains('@'),
            "oauthAccount.email leaked into the account: {json}"
        );
        assert_no_fake("oauthAccount internals", &json);
    }

    #[test]
    fn list_accounts_never_returns_fixture_secret_material() {
        let home = staged_home("home");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let credentials = fs::read_to_string(home.path().join(".claude/.credentials.json"))
            .expect(".credentials.json");
        assert!(
            credentials.contains(FAKE_PREFIX),
            "fixture lost its {FAKE_PREFIX} values; the leak grep would be vacuous"
        );

        assert_no_fake(
            "list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
        assert_no_fake("list_accounts debug", &format!("{accounts:?}"));
        for account in &accounts {
            assert_no_fake("account.id", &account.id);
            assert_no_fake("account.label", &account.label);
            if let Some(identity) = &account.masked_identity {
                assert_no_fake("account.masked_identity", identity);
            }
            if let Some(expires) = &account.expires_at {
                assert_no_fake("account.expires_at", expires);
            }
        }
    }

    #[test]
    fn quota_is_empty_even_when_rate_limit_tier_is_present() {
        let home = staged_home("home");
        let adapter = ClaudeCodeAdapter::with_home(home.path());
        let snapshots = adapter.quota().expect("quota");
        assert!(
            snapshots.is_empty(),
            "rateLimitTier must not be invented into a QuotaSnapshot: {snapshots:?}"
        );
    }

    #[test]
    fn descriptor_is_experimental_while_activate_account_is_unimplemented() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = ClaudeCodeAdapter::with_home(dir.path());
        assert_eq!(adapter.descriptor().maturity, Maturity::Experimental);
        assert!(matches!(
            adapter.activate_account("claude-code-on-disk"),
            Err(Error::NotImplemented("claude-code::activate_account"))
        ));
    }
}
