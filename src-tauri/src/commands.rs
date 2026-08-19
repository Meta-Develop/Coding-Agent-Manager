//! Tauri command surface.
//!
//! This is the only place the webview can call into Rust. Keep commands thin:
//! they translate IPC arguments into core calls and translate errors back. No
//! business logic lives here.

use crate::error::{Error, Result};
use crate::model::{
    Account, AccountListError, AccountListErrorKind, AccountListOutcome, ProviderAccountList,
    ProviderDescriptor, QuotaSnapshot,
};
use crate::providers;

/// Providers whose `list_accounts` inspects only an API key, not OAuth.
///
/// Ceiling: this list is a stand-in for an adapter-reported inspect-scope,
/// which would be a trait change (out of scope for the IPC/UI boundary).
/// Drop `gemini-cli` when that adapter starts reading the OAuth credential
/// file (`docs/research/gemini-cli.md` §2–§3).
const API_KEY_ONLY_LISTING: &[&str] = &["gemini-cli"];

/// Every adapter compiled into this build, with live detection state.
#[tauri::command]
pub fn list_providers() -> Vec<ProviderDescriptor> {
    providers::registry()
        .iter()
        .map(|adapter| adapter.descriptor())
        .collect()
}

/// Accounts for one provider, or for every provider when `provider_id` is null.
///
/// Each adapter produces its own [`ProviderAccountList`]. A failure or an
/// unimplemented adapter is reported on that provider and does not fail the
/// call, so one unfinished or broken adapter cannot blank the UI.
#[tauri::command]
pub fn list_accounts(provider_id: Option<String>) -> Vec<ProviderAccountList> {
    collect_account_listings(provider_id.as_deref())
}

fn collect_account_listings(provider_id: Option<&str>) -> Vec<ProviderAccountList> {
    let adapters = match provider_id {
        Some(id) => providers::find(id).into_iter().collect::<Vec<_>>(),
        None => providers::registry(),
    };
    adapters
        .iter()
        .map(|adapter| listing_from_result(adapter.id(), adapter.list_accounts()))
        .collect()
}

fn listing_from_result(provider_id: &str, result: Result<Vec<Account>>) -> ProviderAccountList {
    match result {
        Ok(accounts) => ProviderAccountList {
            provider_id: provider_id.to_string(),
            accounts,
            outcome: if is_api_key_only_listing(provider_id) {
                AccountListOutcome::ListedApiKeyOnly
            } else {
                AccountListOutcome::Listed
            },
        },
        Err(Error::NotImplemented(_)) => ProviderAccountList {
            provider_id: provider_id.to_string(),
            accounts: Vec::new(),
            outcome: AccountListOutcome::NotImplemented,
        },
        Err(error) => ProviderAccountList {
            provider_id: provider_id.to_string(),
            accounts: Vec::new(),
            outcome: AccountListOutcome::Failed {
                error: account_list_error(&error),
            },
        },
    }
}

fn is_api_key_only_listing(provider_id: &str) -> bool {
    API_KEY_ONLY_LISTING.contains(&provider_id)
}

fn account_list_error(error: &Error) -> AccountListError {
    match error {
        Error::ConfigRead { reason, .. } => AccountListError {
            kind: AccountListErrorKind::ConfigRead,
            path: path_from_reason(reason),
            message: error.to_string(),
        },
        Error::CredentialStoreUnavailable(_) => AccountListError {
            kind: AccountListErrorKind::CredentialStoreUnavailable,
            path: None,
            message: error.to_string(),
        },
        _ => AccountListError {
            kind: AccountListErrorKind::Other,
            path: None,
            message: error.to_string(),
        },
    }
}

/// Adapters that include a path put it first in `ConfigRead.reason`
/// (`/abs/auth.json is not valid JSON`). A bare filename is not a path.
fn path_from_reason(reason: &str) -> Option<String> {
    let token = reason.split([' ', '(']).next().unwrap_or("");
    if token.contains('/') || token.contains('\\') {
        Some(token.to_string())
    } else {
        None
    }
}

fn adapter_for(provider_id: &str) -> Result<Box<dyn providers::ProviderAdapter>> {
    providers::find(provider_id).ok_or_else(|| Error::UnknownProvider(provider_id.to_string()))
}

/// Create a stored account on `provider_id` named `account_id`.
///
/// This command is blocking and interactive. The Codex adapter (the only
/// implementation today) spawns the vendor CLI's own `codex login` with
/// inherited stdio, so it does not return until the user finishes signing
/// in, and the CLI's prompts appear on the terminal that launched this
/// application. The UI has to say so; hiding that would strand a user who
/// never looks at that terminal.
#[tauri::command]
pub fn add_account(provider_id: String, account_id: String) -> Result<()> {
    adapter_for(&provider_id)?.add_account(&account_id)
}

/// Make `account_id` the account `provider_id`'s tool will use on its next start.
///
/// Account ids are per-provider. The caller must name the adapter; scanning
/// every adapter for an id is both wasteful and ambiguous.
#[tauri::command]
pub fn activate_account(provider_id: String, account_id: String) -> Result<()> {
    adapter_for(&provider_id)?.activate_account(&account_id)
}

/// Forget a stored account. Does not sign the user out of the live tool.
#[tauri::command]
pub fn delete_account(provider_id: String, account_id: String) -> Result<()> {
    adapter_for(&provider_id)?.delete_account(&account_id)
}

/// Quota snapshots from every adapter that publishes one.
///
/// Same swallow as the old `list_accounts`: a single adapter error is
/// dropped and the rest still arrive. Left consistent-but-wrong rather than
/// given the per-provider outcome shape, because every current adapter
/// returns `Ok([])` from `quota()` and the dashboard still does not render
/// quota (`FR-5`). When an adapter first returns a real quota error, this
/// command should grow the same outcome contract as `list_accounts`. Silence
/// would hide that decision.
#[tauri::command]
pub fn list_quota() -> Result<Vec<QuotaSnapshot>> {
    let mut snapshots = Vec::new();
    for adapter in providers::registry() {
        if let Ok(found) = adapter.quota() {
            snapshots.extend(found);
        }
    }
    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Account, AuthKind};

    fn sample_account(provider_id: &str) -> Account {
        Account {
            id: "test-account".to_string(),
            provider_id: provider_id.to_string(),
            label: "Test".to_string(),
            masked_identity: Some("****0001".to_string()),
            auth_kind: AuthKind::ApiKey,
            is_active: true,
            is_stored: false,
            expires_at: None,
        }
    }

    #[test]
    fn listed_empty_is_listed_not_a_failure() {
        let listing = listing_from_result("codex-cli", Ok(Vec::new()));
        assert_eq!(listing.provider_id, "codex-cli");
        assert!(listing.accounts.is_empty());
        assert_eq!(listing.outcome, AccountListOutcome::Listed);
    }

    #[test]
    fn listed_accounts_are_preserved() {
        let listing = listing_from_result("codex-cli", Ok(vec![sample_account("codex-cli")]));
        assert_eq!(listing.accounts.len(), 1);
        assert_eq!(listing.outcome, AccountListOutcome::Listed);
    }

    #[test]
    fn gemini_empty_is_listed_api_key_only() {
        let listing = listing_from_result("gemini-cli", Ok(Vec::new()));
        assert!(listing.accounts.is_empty());
        assert_eq!(listing.outcome, AccountListOutcome::ListedApiKeyOnly);
    }

    #[test]
    fn gemini_with_a_key_stays_listed_api_key_only() {
        let listing = listing_from_result("gemini-cli", Ok(vec![sample_account("gemini-cli")]));
        assert_eq!(listing.accounts.len(), 1);
        assert_eq!(listing.outcome, AccountListOutcome::ListedApiKeyOnly);
    }

    #[test]
    fn not_implemented_is_its_own_outcome() {
        let listing = listing_from_result(
            "cursor",
            Err(Error::NotImplemented("cursor::list_accounts")),
        );
        assert!(listing.accounts.is_empty());
        assert_eq!(listing.outcome, AccountListOutcome::NotImplemented);
    }

    #[test]
    fn config_read_carries_kind_path_and_message_without_inventing_a_secret() {
        let error = Error::ConfigRead {
            provider: "codex-cli".to_string(),
            reason: "/tmp/cam-test/auth.json is not valid JSON".to_string(),
        };
        let listing = listing_from_result("codex-cli", Err(error));
        assert!(listing.accounts.is_empty());
        let AccountListOutcome::Failed { error } = listing.outcome else {
            panic!("expected Failed, got {:?}", listing.outcome);
        };
        assert_eq!(error.kind, AccountListErrorKind::ConfigRead);
        assert_eq!(error.path.as_deref(), Some("/tmp/cam-test/auth.json"));
        assert_eq!(
            error.message,
            "configuration for `codex-cli` could not be read: /tmp/cam-test/auth.json is not valid JSON"
        );
        assert!(
            !error.message.contains("FAKE-"),
            "surfaced error leaked fixture secret material: {}",
            error.message
        );
    }

    #[test]
    fn config_read_without_a_path_leaves_path_unset() {
        let error = Error::ConfigRead {
            provider: "grok-cli".to_string(),
            reason: "auth.json is not valid JSON".to_string(),
        };
        let listing = listing_from_result("grok-cli", Err(error));
        let AccountListOutcome::Failed { error } = listing.outcome else {
            panic!("expected Failed, got {:?}", listing.outcome);
        };
        assert_eq!(error.kind, AccountListErrorKind::ConfigRead);
        assert_eq!(error.path, None);
    }

    #[test]
    fn credential_store_unavailable_has_its_own_kind() {
        let listing = listing_from_result(
            "codex-cli",
            Err(Error::CredentialStoreUnavailable("locked".to_string())),
        );
        let AccountListOutcome::Failed { error } = listing.outcome else {
            panic!("expected Failed, got {:?}", listing.outcome);
        };
        assert_eq!(error.kind, AccountListErrorKind::CredentialStoreUnavailable);
        assert_eq!(error.path, None);
        assert!(error.message.contains("credential store is unavailable"));
    }

    #[test]
    fn other_errors_map_to_other() {
        let listing = listing_from_result(
            "codex-cli",
            Err(Error::UnknownAccount("missing".to_string())),
        );
        let AccountListOutcome::Failed { error } = listing.outcome else {
            panic!("expected Failed, got {:?}", listing.outcome);
        };
        assert_eq!(error.kind, AccountListErrorKind::Other);
    }

    #[test]
    fn one_failed_listing_does_not_prevent_collecting_another() {
        // Classification is per result: a Failed and a Listed can sit in the
        // same vec. The registry walk is not called here — it would read the
        // real home directory.
        let listings = [
            listing_from_result(
                "codex-cli",
                Err(Error::ConfigRead {
                    provider: "codex-cli".to_string(),
                    reason: "/tmp/cam-test/auth.json is not valid JSON".to_string(),
                }),
            ),
            listing_from_result("claude-code", Ok(vec![sample_account("claude-code")])),
        ];
        assert!(matches!(
            listings[0].outcome,
            AccountListOutcome::Failed { .. }
        ));
        assert_eq!(listings[1].accounts.len(), 1);
        assert_eq!(listings[1].outcome, AccountListOutcome::Listed);
    }

    #[test]
    fn unknown_provider_id_yields_no_listings() {
        assert!(collect_account_listings(Some("not-a-provider")).is_empty());
    }

    #[test]
    fn add_account_unknown_provider_is_unknown_provider() {
        let error =
            add_account("not-a-provider".into(), "acct-work".into()).expect_err("unknown provider");
        assert!(matches!(
            error,
            Error::UnknownProvider(ref id) if id == "not-a-provider"
        ));
    }

    #[test]
    fn activate_account_unknown_provider_is_unknown_provider() {
        let error = activate_account("not-a-provider".into(), "acct-work".into())
            .expect_err("unknown provider");
        assert!(matches!(
            error,
            Error::UnknownProvider(ref id) if id == "not-a-provider"
        ));
    }

    #[test]
    fn delete_account_unknown_provider_is_unknown_provider() {
        let error = delete_account("not-a-provider".into(), "acct-work".into())
            .expect_err("unknown provider");
        assert!(matches!(
            error,
            Error::UnknownProvider(ref id) if id == "not-a-provider"
        ));
    }

    #[test]
    fn mutating_commands_on_an_unimplemented_adapter_are_not_implemented() {
        // Cursor's methods are stubs: they return NotImplemented without
        // touching the real home, keychain, or data directory.
        let add = add_account("cursor".into(), "acct-work".into()).expect_err("cursor add");
        let activate =
            activate_account("cursor".into(), "acct-work".into()).expect_err("cursor activate");
        let delete =
            delete_account("cursor".into(), "acct-work".into()).expect_err("cursor delete");
        assert!(matches!(add, Error::NotImplemented(_)), "got {add:?}");
        assert!(
            matches!(activate, Error::NotImplemented(_)),
            "got {activate:?}"
        );
        assert!(matches!(delete, Error::NotImplemented(_)), "got {delete:?}");
    }
}
