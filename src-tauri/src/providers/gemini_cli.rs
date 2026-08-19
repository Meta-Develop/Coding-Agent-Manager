//! Gemini CLI (Google) adapter.
//!
//! Observed layout on Linux, gemini 0.47.0:
//!
//! - `~/.gemini/projects.json` [verified-local] — a `projects` map; it was
//!   empty on the inspected host.
//!
//! No credential file was present, because the inspected installation had not
//! completed a sign-in. The OAuth credential path and format are `[unknown]`.
//! This adapter implements only the documented API-key path
//! (`GEMINI_API_KEY`, [verified-docs]). It does not search the filesystem for
//! a credential file, does not guess a filename, and does not implement
//! `activate_account`.
//!
//! TODO(research): confirm the OAuth credential path and the settings file
//! location. See `docs/research/gemini-cli.md`.

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor, QuotaSnapshot};

const PROVIDER_ID: &str = "gemini-cli";

/// Application-assigned id for the single `GEMINI_API_KEY` identity.
///
/// There is one environment variable, so there is one slot. The id names
/// that slot rather than echoing or hashing the key (`docs/SPEC.md` §4;
/// NFR-1 — the id travels to the webview).
const API_KEY_ACCOUNT_ID: &str = "gemini-cli-api-key";

const API_KEY_ENV: &str = "GEMINI_API_KEY";

#[derive(Debug, Default)]
pub struct GeminiCliAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
    /// Injected `GEMINI_API_KEY`. `None` means read the process environment,
    /// which is what production uses. Tests set `Some(...)` so they never
    /// call `std::env::set_var` (Rust runs tests in parallel in one process)
    /// and never observe a real key (`docs/TESTING.md` §4). An injected
    /// empty string is treated as no key.
    api_key: Option<String>,
}

impl GeminiCliAdapter {
    /// Root this adapter at `home` instead of the real user home.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
            ..Self::default()
        }
    }

    /// Override `GEMINI_API_KEY` without touching the process environment.
    #[cfg(test)]
    fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    fn resolved_home(&self) -> Option<PathBuf> {
        self.home.clone().or_else(home_dir)
    }

    /// Resolve the API key this adapter should treat as present.
    ///
    /// An injected override always wins. An injected home with no override
    /// does not fall through to the process environment: tests root at a
    /// `TempDir` and must not observe a developer's `GEMINI_API_KEY`.
    /// Production leaves both fields unset, so the documented variable is
    /// read. Empty strings are not a key.
    fn resolved_api_key(&self) -> Option<String> {
        if let Some(injected) = &self.api_key {
            return (!injected.is_empty()).then(|| injected.clone());
        }
        if self.home.is_some() {
            return None;
        }
        std::env::var(API_KEY_ENV)
            .ok()
            .filter(|value| !value.is_empty())
    }
}

/// Mask an API key for display.
///
/// Keeps the last four characters and replaces the rest with a fixed `****`
/// prefix (e.g. `****0001`). Returns `None` when the value is empty or too
/// short to mask without leaving the original essentially intact.
fn mask_api_key(raw: &str) -> Option<String> {
    const VISIBLE: usize = 4;
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= VISIBLE {
        return None;
    }
    let tail: String = chars[chars.len() - VISIBLE..].iter().collect();
    Some(format!("****{tail}"))
}

impl ProviderAdapter for GeminiCliAdapter {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Gemini CLI".to_string(),
            vendor: "Google".to_string(),
            // `auth_kinds` describes the *provider*, not this adapter.
            // Gemini CLI documents both OAuth and API-key [verified-docs].
            // This adapter implements only the API-key path; OAuth stays
            // unimplemented until the credential file is [verified-local].
            // `Maturity::Experimental` is the honest adapter signal (NFR-8).
            // Dropping `OAuth` here would claim the provider cannot do
            // OAuth, which is false.
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            // Experimental: `list_accounts` works for the API-key path,
            // `activate_account` does not. `Supported` would overstate the
            // adapter (NFR-8).
            maturity: Maturity::Experimental,
            install_state: self.detect(),
            capabilities: Vec::new(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(home) = self.resolved_home() else {
            return Vec::new();
        };
        let root = home.join(".gemini");
        vec![root.join("projects.json"), root.join("settings.json")]
    }

    fn detect(&self) -> InstallState {
        let has_config = self
            .resolved_home()
            .map(|home| home.join(".gemini").is_dir())
            .unwrap_or(false);
        if binary_on_path("gemini") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        let Some(key) = self.resolved_api_key() else {
            return Ok(Vec::new());
        };
        Ok(vec![Account {
            id: API_KEY_ACCOUNT_ID.to_string(),
            provider_id: PROVIDER_ID.to_string(),
            label: String::new(),
            masked_identity: mask_api_key(&key),
            auth_kind: AuthKind::ApiKey,
            // The environment variable is what the tool will use
            // [verified-docs]. This is the only account this adapter can
            // see, and it is the value the CLI reads. `is_active` is
            // therefore `true`.
            is_active: true,
            // GEMINI_API_KEY is the tool's environment, not a stored copy
            // this application created.
            is_stored: false,
            // An API key has no observable expiry here.
            expires_at: None,
        }])
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        // Switching an API-key account is environmental: set `GEMINI_API_KEY`
        // on the launched process [verified-docs]. That is the safest switch
        // in the initial set — it touches no file — but it changes the
        // environment of processes this application launches, and that
        // launcher does not exist yet. Implementing half of it now would be
        // a write path with no consumer.
        Err(Error::NotImplemented("gemini-cli::activate_account"))
    }

    fn quota(&self) -> Result<Vec<QuotaSnapshot>> {
        // Local quota is `[unknown]` (`docs/research/gemini-cli.md` §6).
        // NFR-8 forbids inventing a number.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;

    /// Fixture key. `FAKE-` prefix, no vendor-shaped `AIza` prefix.
    const TEST_KEY: &str = "FAKE-gemini-key-0001";

    fn adapter_with_key(key: Option<&str>) -> (tempfile::TempDir, GeminiCliAdapter) {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = match key {
            Some(key) => GeminiCliAdapter::with_home(dir.path()).with_api_key(key),
            None => GeminiCliAdapter::with_home(dir.path()),
        };
        (dir, adapter)
    }

    fn assert_no_fake(where_: &str, text: &str) {
        assert!(
            !text.contains("FAKE-"),
            "{where_} leaked fixture secret material: {text}"
        );
    }

    #[test]
    fn with_home_resolves_config_paths_under_the_injected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = GeminiCliAdapter::with_home(dir.path());
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
    fn mask_api_key_keeps_only_the_last_four_characters() {
        assert_eq!(mask_api_key("ab12cd34"), Some("****cd34".to_string()));
        assert_eq!(mask_api_key(TEST_KEY), Some("****0001".to_string()));
        assert_eq!(mask_api_key("wxyz"), None);
        assert_eq!(mask_api_key("abc"), None);
        assert_eq!(mask_api_key(""), None);

        let masked = mask_api_key(TEST_KEY).expect("maskable");
        assert_ne!(masked, TEST_KEY);
        assert!(!masked.contains(TEST_KEY));
    }

    #[test]
    fn list_accounts_returns_one_active_api_key_account_when_a_key_is_set() {
        let (_tmp, adapter) = adapter_with_key(Some(TEST_KEY));
        let accounts = adapter.list_accounts().expect("list_accounts");

        assert_eq!(accounts.len(), 1);
        let account = &accounts[0];
        assert_eq!(account.id, API_KEY_ACCOUNT_ID);
        assert_eq!(account.provider_id, PROVIDER_ID);
        assert_eq!(account.auth_kind, AuthKind::ApiKey);
        assert!(account.is_active);
        assert_eq!(account.expires_at, None);
        assert_eq!(account.masked_identity.as_deref(), Some("****0001"));
        assert!(!account.id.contains(TEST_KEY));
        assert!(!account
            .masked_identity
            .as_deref()
            .expect("masked")
            .contains(TEST_KEY));

        let again = adapter.list_accounts().expect("second list");
        assert_eq!(again[0].id, account.id);

        assert_no_fake(
            "list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
        assert_no_fake("list_accounts debug", &format!("{accounts:?}"));
    }

    #[test]
    fn list_accounts_returns_empty_when_no_key() {
        let (_tmp, adapter) = adapter_with_key(None);
        let accounts = adapter.list_accounts().expect("no key is empty");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_treats_empty_string_key_as_no_key() {
        let (_tmp, adapter) = adapter_with_key(Some(""));
        let accounts = adapter.list_accounts().expect("empty key is empty");
        assert!(accounts.is_empty());
    }

    #[test]
    fn masked_identity_never_contains_the_key() {
        let (_tmp, adapter) = adapter_with_key(Some(TEST_KEY));
        let accounts = adapter.list_accounts().expect("list_accounts");
        let masked = accounts[0]
            .masked_identity
            .as_deref()
            .expect("a long key is maskable");
        assert!(!masked.contains(TEST_KEY));
        assert_no_fake("masked_identity", masked);
    }

    #[test]
    fn account_id_does_not_contain_the_key() {
        let (_tmp, adapter) = adapter_with_key(Some(TEST_KEY));
        let accounts = adapter.list_accounts().expect("list_accounts");
        assert!(!accounts[0].id.contains(TEST_KEY));
        assert_no_fake("account.id", &accounts[0].id);
        assert_eq!(accounts[0].id, API_KEY_ACCOUNT_ID);
    }

    #[test]
    fn quota_is_empty() {
        let (_tmp, adapter) = adapter_with_key(Some(TEST_KEY));
        let quota = adapter.quota().expect("quota");
        assert!(quota.is_empty());
    }

    #[test]
    fn descriptor_is_experimental_while_activate_account_is_unimplemented() {
        let (_tmp, adapter) = adapter_with_key(None);
        assert_eq!(adapter.descriptor().maturity, Maturity::Experimental);
        assert!(matches!(
            adapter.activate_account("unused"),
            Err(Error::NotImplemented("gemini-cli::activate_account"))
        ));
    }
}
