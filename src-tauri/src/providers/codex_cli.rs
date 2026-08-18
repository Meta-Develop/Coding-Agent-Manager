//! Codex CLI (OpenAI) adapter.
//!
//! Observed layout on Linux, codex-cli 0.144.4:
//!
//! - `~/.codex/auth.json` [verified-local] — top-level `auth_mode`,
//!   `OPENAI_API_KEY` (null while signed in through a ChatGPT plan), a `tokens`
//!   object with `id_token` / `access_token` / `refresh_token` / `account_id`,
//!   and `last_refresh`.
//! - `~/.codex/config.toml` [verified-local] — client configuration including
//!   per-project `[projects."<path>"]` trust entries.
//!
//! Because `auth.json` is a single self-contained document, Codex is the
//! cleanest switching target of the initial five: a switch is a validated
//! replacement of that one file. `CODEX_HOME` relocates the whole directory
//! [verified-docs], which gives a second, less invasive switching strategy.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

/// Application-assigned id for the single on-disk Codex identity.
///
/// Codex stores exactly one identity in one flat document [verified-local], so
/// this names that slot rather than echoing a vendor identifier. SPEC §4
/// assigns `Account.id` here so a vendor changing `tokens.account_id` cannot
/// orphan local state. The same fixture therefore always produces this id.
const ON_DISK_ACCOUNT_ID: &str = "codex-cli-on-disk";

#[derive(Debug, Default)]
pub struct CodexCliAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
}

impl CodexCliAdapter {
    /// Root this adapter at `home` instead of the real user home.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
        }
    }

    fn resolved_home(&self) -> Option<PathBuf> {
        self.home.clone().or_else(home_dir)
    }

    /// Honours `CODEX_HOME` before falling back to `~/.codex`.
    ///
    /// An injected root wins over `CODEX_HOME` so a test is never affected by
    /// the developer's own `CODEX_HOME` (`docs/TESTING.md` §4). Production
    /// leaves `home` unset, so the documented `[verified-docs]` override still
    /// precedes `~/.codex`. Order: injected root, then `CODEX_HOME`, then
    /// `~/.codex`.
    fn codex_home(&self) -> Option<PathBuf> {
        if self.home.is_some() {
            return self.resolved_home().map(|home| home.join(".codex"));
        }
        if let Some(explicit) = std::env::var_os("CODEX_HOME") {
            return Some(PathBuf::from(explicit));
        }
        self.resolved_home().map(|home| home.join(".codex"))
    }

    fn config_read(&self, reason: impl Into<String>) -> Error {
        Error::ConfigRead {
            provider: self.id().to_string(),
            reason: reason.into(),
        }
    }
}

/// Mask a vendor identifier for display.
///
/// Keeps the last four characters and replaces the rest with a fixed `****`
/// prefix (e.g. `****ab12`). Returns `None` when the value is absent-as-empty
/// or too short to mask without leaving the original essentially intact.
///
/// The only identity-shaped field in `auth.json` is `tokens.account_id`
/// [verified-local]. Callers must pass that field and never a token or key.
fn mask_identity(raw: &str) -> Option<String> {
    const VISIBLE: usize = 4;
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= VISIBLE {
        return None;
    }
    let tail: String = chars[chars.len() - VISIBLE..].iter().collect();
    Some(format!("****{tail}"))
}

/// Build the single `Account` that a parsed `auth.json` object represents.
///
/// The document holds exactly one identity [verified-local], so whatever is in
/// it is what the tool will use — `is_active` is therefore always `true`.
///
/// Classification inspects structure only (NFR-1 / threat T2):
/// - `ApiKey` when `OPENAI_API_KEY` is a non-null string [verified-local]
/// - otherwise `OAuth` when a `tokens` object is present [verified-local]
/// - otherwise `AuthKind::Unknown`
///
/// Token, key, and raw `account_id` values are never copied onto the `Account`.
/// `id_token` is a JWT and is not decoded.
fn account_from_auth(
    provider_id: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Account {
    let has_api_key = matches!(
        object.get("OPENAI_API_KEY"),
        Some(serde_json::Value::String(_))
    );
    let tokens = object.get("tokens").and_then(serde_json::Value::as_object);
    let auth_kind = if has_api_key {
        AuthKind::ApiKey
    } else if tokens.is_some() {
        AuthKind::OAuth
    } else {
        AuthKind::Unknown
    };
    let masked_identity = tokens
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(mask_identity);

    Account {
        id: ON_DISK_ACCOUNT_ID.to_string(),
        provider_id: provider_id.to_string(),
        label: "Codex CLI".to_string(),
        masked_identity,
        auth_kind,
        is_active: true,
        // `last_refresh` is a refresh timestamp, not an expiry [verified-local].
        // Inventing `expires_at` from it would fabricate a capability the file
        // does not provide (NFR-8).
        expires_at: None,
    }
}

impl ProviderAdapter for CodexCliAdapter {
    fn id(&self) -> &'static str {
        "codex-cli"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Codex CLI".to_string(),
            vendor: "OpenAI".to_string(),
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            // Read-only listing works; switching is still NotImplemented.
            // `Supported` would overstate that (NFR-8).
            maturity: Maturity::Experimental,
            install_state: self.detect(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(root) = self.codex_home() else {
            return Vec::new();
        };
        vec![root.join("auth.json"), root.join("config.toml")]
    }

    fn detect(&self) -> InstallState {
        let has_config = self.codex_home().map(|dir| dir.is_dir()).unwrap_or(false);
        if binary_on_path("codex") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        let Some(path) = self.codex_home().map(|root| root.join("auth.json")) else {
            return Ok(Vec::new());
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            // A missing file means no account is configured [verified-local].
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(self.config_read(format!("{} ({})", path.display(), error.kind())));
            }
        };
        // Reason strings name the path and the kind of failure only. The serde
        // error text can echo a token from the file; never include it (NFR-1).
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| self.config_read(format!("{} is not valid JSON", path.display())))?;
        let Some(object) = value.as_object() else {
            return Err(self.config_read(format!("{} is not a JSON object", path.display())));
        };
        Ok(vec![account_from_auth(self.id(), object)])
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("codex-cli::activate_account"))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;
    use std::fs;
    use std::path::Path;

    const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-cli");
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

    #[test]
    fn with_home_resolves_config_paths_under_the_injected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CodexCliAdapter::with_home(dir.path());
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
    fn list_accounts_matches_the_oauth_fixture_expectation() {
        let home = staged_home("home");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let expected_path = Path::new(FIXTURE_ROOT).join("expected/accounts.json");
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&expected_path).expect("expected/accounts.json"),
        )
        .expect("expected json");
        let got = serde_json::to_value(&accounts).expect("serialize");
        assert_eq!(got, expected);

        assert_eq!(accounts.len(), 1);
        assert!(accounts[0].is_active);
        assert_eq!(accounts[0].auth_kind, AuthKind::OAuth);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        assert_eq!(accounts[0].expires_at, None);
        assert_eq!(accounts[0].id, ON_DISK_ACCOUNT_ID);

        let again = adapter.list_accounts().expect("second list");
        assert_eq!(again[0].id, accounts[0].id);
    }

    #[test]
    fn list_accounts_classifies_a_non_null_api_key_as_api_key() {
        let home = staged_home("home-api-key");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].auth_kind, AuthKind::ApiKey);
        assert!(accounts[0].is_active);
        assert_eq!(accounts[0].expires_at, None);
        assert_eq!(accounts[0].masked_identity, None);
        assert_no_fake(
            "api-key list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
    }

    #[test]
    fn list_accounts_returns_empty_when_auth_json_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CodexCliAdapter::with_home(dir.path());
        let accounts = adapter
            .list_accounts()
            .expect("a missing auth.json is not an error");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_rejects_malformed_auth_json() {
        let home = staged_home("home-malformed");
        let adapter = CodexCliAdapter::with_home(home.path());
        let error = adapter
            .list_accounts()
            .expect_err("unparsable auth.json must not be guessed at");
        assert!(
            matches!(error, Error::ConfigRead { .. }),
            "expected ConfigRead, got {error:?}"
        );
        assert_no_fake("ConfigRead Display", &error.to_string());
        assert_no_fake("ConfigRead Debug", &format!("{error:?}"));
    }

    #[test]
    fn list_accounts_never_returns_fixture_secret_material() {
        let home = staged_home("home");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let auth = fs::read_to_string(home.path().join(".codex/auth.json")).expect("auth.json");
        assert!(
            auth.contains(FAKE_PREFIX),
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
        }
    }

    #[test]
    fn descriptor_is_experimental_while_activate_account_is_unimplemented() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CodexCliAdapter::with_home(dir.path());
        assert_eq!(adapter.descriptor().maturity, Maturity::Experimental);
        assert!(matches!(
            adapter.activate_account("codex-cli-on-disk"),
            Err(Error::NotImplemented("codex-cli::activate_account"))
        ));
    }
}
