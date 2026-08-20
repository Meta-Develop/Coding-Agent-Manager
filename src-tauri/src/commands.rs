//! Tauri command surface.
//!
//! This is the only place the webview can call into Rust. Keep commands thin:
//! they translate IPC arguments into core calls and translate errors back. No
//! business logic lives here.

use crate::error::{Error, Result};
use crate::model::{
    Account, AccountListError, AccountListErrorKind, AccountListOutcome, LaunchedProcess,
    ProviderAccountList, ProviderDescriptor, ProviderQuotaList, QuotaListError, QuotaListErrorKind,
    QuotaListOutcome, QuotaSnapshot, RelayStatus,
};
use crate::providers;
use crate::providers::codex_cli::CodexCliAdapter;
use crate::providers::{ActivationMechanism, ProviderAdapter, StoredAccountRegistry};
use crate::relay;
use crate::storage;

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
        .map(|adapter| listing_for(adapter.as_ref()))
        .collect()
}

/// Codex can enumerate stored copies when the live document is damaged.
/// The trait still returns `Result<Vec<Account>>`, so a listing+warning
/// has to be recovered from the concrete type. Other adapters keep the
/// existing `Ok` / `Err` mapping. A trait method that returned both
/// would remove this branch.
fn listing_for(adapter: &dyn ProviderAdapter) -> ProviderAccountList {
    if adapter.id() == "codex-cli" {
        return listing_from_detailed(
            "codex-cli",
            CodexCliAdapter::default().list_accounts_detailed(),
        );
    }
    listing_from_result(adapter.id(), adapter.list_accounts())
}

fn listing_from_detailed(
    provider_id: &str,
    result: Result<(Vec<Account>, Option<Error>)>,
) -> ProviderAccountList {
    match result {
        Ok((accounts, Some(error))) => ProviderAccountList {
            provider_id: provider_id.to_string(),
            accounts,
            outcome: AccountListOutcome::ListedWithError {
                error: account_list_error(&error),
            },
        },
        Ok((accounts, None)) => listing_from_result(provider_id, Ok(accounts)),
        Err(error) => listing_from_result(provider_id, Err(error)),
    }
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
/// Legacy adapters own their transaction. Core-managed adapters persist
/// pending/complete metadata around native provisioning and send any returned
/// `Secret` directly to `CredentialStore`; no secret is an IPC argument.
#[tauri::command]
pub fn add_account(provider_id: String, account_id: String) -> Result<()> {
    let adapter = adapter_for(&provider_id)?;
    let Some(plan) = adapter.managed_account_plan() else {
        return adapter.add_account(&account_id);
    };
    let store = (plan.material == crate::model::StoredAccountMaterial::CredentialStore)
        .then(storage::default_store)
        .transpose()?;
    providers::add_managed_account(
        &stored_account_registry()?,
        adapter.as_ref(),
        &account_id,
        &account_id,
        store.as_deref(),
    )
}

/// Make `account_id` the account `provider_id`'s tool will use on its next start.
///
/// Account ids are per-provider. The caller must name the adapter; scanning
/// every adapter for an id is both wasteful and ambiguous.
#[tauri::command]
pub fn activate_account(provider_id: String, account_id: String) -> Result<()> {
    let adapter = adapter_for(&provider_id)?;
    match adapter.activation_mechanism() {
        ActivationMechanism::ToolConfiguration => adapter.activate_account(&account_id),
        ActivationMechanism::LaunchEnvironment => providers::select_launch_account(
            &stored_account_registry()?,
            adapter.as_ref(),
            &account_id,
        ),
    }
}

/// Forget a stored account. Core-managed deletion clears selection first,
/// deletes credential-store material when present, and retains vendor homes.
#[tauri::command]
pub fn delete_account(provider_id: String, account_id: String) -> Result<()> {
    let adapter = adapter_for(&provider_id)?;
    let Some(plan) = adapter.managed_account_plan() else {
        return adapter.delete_account(&account_id);
    };
    let store = (plan.material == crate::model::StoredAccountMaterial::CredentialStore)
        .then(storage::default_store)
        .transpose()?;
    providers::delete_managed_account(
        &stored_account_registry()?,
        adapter.as_ref(),
        &account_id,
        store.as_deref(),
    )
}

/// Start a provider tool with the environment of its selected stored account.
///
/// The webview supplies no environment or credential value. The adapter
/// declares the command; core resolves any credential only at spawn time and
/// returns non-secret process metadata.
#[tauri::command]
pub fn launch_provider(provider_id: String) -> Result<LaunchedProcess> {
    let adapter = adapter_for(&provider_id)?;
    if adapter.activation_mechanism() != ActivationMechanism::LaunchEnvironment {
        return Err(Error::NotImplemented("launch_provider"));
    }
    let account = stored_account_registry()?
        .selected(&provider_id)?
        .ok_or_else(|| Error::UnknownAccount(provider_id.clone()))?;
    let spec = providers::launch_spec_for(adapter.as_ref(), &account)?;
    let store = spec
        .requires_credential()
        .then(storage::default_store)
        .transpose()?;
    let mut child = providers::spawn_launch(spec, &account, store.as_deref())?;
    let process = LaunchedProcess {
        provider_id,
        account_id: account.id,
        process_id: child.id(),
    };
    // Waiting outside the command keeps an exited CLI from becoming a zombie
    // while allowing the IPC call to return as soon as the process starts.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(process)
}

fn stored_account_registry() -> Result<StoredAccountRegistry> {
    let dirs = crate::paths::project_dirs().ok_or_else(|| Error::ConfigRead {
        provider: "account-metadata".to_string(),
        reason: "the application data directory could not be resolved".to_string(),
    })?;
    Ok(StoredAccountRegistry::new(
        crate::paths::stored_accounts_path(dirs.data_dir()),
    ))
}

/// One honest quota result for every registered provider.
///
/// Empty adapter output is an explicit `no-signal`, while collection errors
/// stay scoped to their provider. Snapshots are validated before IPC so an
/// adapter contract violation can never become a visible number (FR-5, NFR-8).
#[tauri::command]
pub fn list_quota() -> Vec<ProviderQuotaList> {
    providers::registry()
        .iter()
        .map(|adapter| quota_listing_for(adapter.as_ref()))
        .collect()
}

fn quota_listing_for(adapter: &dyn ProviderAdapter) -> ProviderQuotaList {
    quota_listing_from_results(adapter.id(), adapter.quota(), adapter.plan_label())
}

fn quota_listing_from_results(
    provider_id: &str,
    snapshots: Result<Vec<QuotaSnapshot>>,
    plan_label: Result<Option<String>>,
) -> ProviderQuotaList {
    let snapshots = match snapshots {
        Ok(snapshots) => snapshots,
        Err(error) => return failed_quota_listing(provider_id, quota_list_error(&error)),
    };
    let plan_label = match plan_label {
        Ok(plan_label) => plan_label,
        Err(error) => return failed_quota_listing(provider_id, quota_list_error(&error)),
    };
    if let Some(error) = invalid_snapshot_error(&snapshots) {
        return failed_quota_listing(provider_id, error);
    }
    let outcome = if snapshots.is_empty() {
        QuotaListOutcome::NoSignal
    } else {
        QuotaListOutcome::Available
    };
    ProviderQuotaList {
        provider_id: provider_id.to_string(),
        plan_label,
        snapshots,
        outcome,
    }
}

fn invalid_snapshot_error(snapshots: &[QuotaSnapshot]) -> Option<QuotaListError> {
    for snapshot in snapshots {
        if !snapshot.utilization.is_finite() || !(0.0..=1.0).contains(&snapshot.utilization) {
            return Some(QuotaListError {
                kind: QuotaListErrorKind::InvalidSnapshot,
                path: None,
                message: "adapter returned quota utilization outside 0..=1".to_string(),
            });
        }
        if !is_rfc3339(&snapshot.captured_at) {
            return Some(QuotaListError {
                kind: QuotaListErrorKind::InvalidSnapshot,
                path: None,
                message: "adapter returned quota with invalid capturedAt".to_string(),
            });
        }
        if snapshot
            .resets_at
            .as_deref()
            .is_some_and(|resets_at| !is_rfc3339(resets_at))
        {
            return Some(QuotaListError {
                kind: QuotaListErrorKind::InvalidSnapshot,
                path: None,
                message: "adapter returned quota with invalid resetsAt".to_string(),
            });
        }
    }
    None
}

fn is_rfc3339(timestamp: &str) -> bool {
    time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339).is_ok()
}

fn failed_quota_listing(provider_id: &str, error: QuotaListError) -> ProviderQuotaList {
    ProviderQuotaList {
        provider_id: provider_id.to_string(),
        plan_label: None,
        snapshots: Vec::new(),
        outcome: QuotaListOutcome::Failed { error },
    }
}

fn quota_list_error(error: &Error) -> QuotaListError {
    match error {
        Error::ConfigRead { reason, .. } => QuotaListError {
            kind: QuotaListErrorKind::ConfigRead,
            path: path_from_reason(reason),
            message: error.to_string(),
        },
        _ => QuotaListError {
            kind: QuotaListErrorKind::Other,
            path: None,
            message: error.to_string(),
        },
    }
}

/// Start the relay with its safe default loopback configuration.
///
/// No listener configuration or authentication token crosses the IPC boundary.
#[tauri::command]
pub async fn start_relay() -> Result<RelayStatus> {
    relay::start_relay().await
}

/// Stop the relay listener and return its resulting state.
#[tauri::command]
pub async fn stop_relay() -> Result<RelayStatus> {
    relay::stop_relay().await
}

/// Return public listener state. The relay core owns the configured prefixes.
#[tauri::command]
pub async fn relay_status() -> Result<RelayStatus> {
    relay::relay_status().await
}

/// Start the desktop application. Tauri remains confined to this IPC module.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_providers,
            list_accounts,
            add_account,
            activate_account,
            delete_account,
            launch_provider,
            list_quota,
            start_relay,
            stop_relay,
            relay_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coding Agent Manager");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Account, AuthKind, QuotaSource};

    fn sample_account(provider_id: &str) -> Account {
        Account {
            id: "test-account".to_string(),
            provider_id: provider_id.to_string(),
            label: "Test".to_string(),
            masked_identity: Some("****0001".to_string()),
            auth_kind: AuthKind::ApiKey,
            is_active: true,
            is_selected_for_launch: false,
            is_stored: false,
            is_incomplete: false,
            expires_at: None,
        }
    }

    fn sample_quota() -> QuotaSnapshot {
        QuotaSnapshot {
            account_id: "test-account".to_string(),
            model: None,
            utilization: 0.25,
            window_label: Some("5 hours".to_string()),
            resets_at: None,
            captured_at: "2030-01-01T00:00:00Z".to_string(),
            source: QuotaSource::LocalFile,
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
    fn listed_with_error_keeps_accounts_and_the_live_error() {
        let listing = listing_from_detailed(
            "codex-cli",
            Ok((
                vec![sample_account("codex-cli")],
                Some(Error::ConfigRead {
                    provider: "codex-cli".to_string(),
                    reason: "/tmp/cam-test/auth.json is not valid JSON".to_string(),
                }),
            )),
        );
        assert_eq!(listing.accounts.len(), 1);
        let AccountListOutcome::ListedWithError { error } = listing.outcome else {
            panic!("expected ListedWithError, got {:?}", listing.outcome);
        };
        assert_eq!(error.kind, AccountListErrorKind::ConfigRead);
        assert_eq!(error.path.as_deref(), Some("/tmp/cam-test/auth.json"));
        assert!(
            !error.message.contains("FAKE-"),
            "surfaced error leaked fixture secret material: {}",
            error.message
        );
    }

    #[test]
    fn listed_with_no_live_error_stays_listed() {
        let listing = listing_from_detailed("codex-cli", Ok((Vec::new(), None)));
        assert!(listing.accounts.is_empty());
        assert_eq!(listing.outcome, AccountListOutcome::Listed);
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
    fn available_quota_preserves_sourced_snapshots() {
        let listing = quota_listing_from_results(
            "example",
            Ok(vec![sample_quota()]),
            Ok(Some("Example plan".to_string())),
        );
        assert_eq!(listing.outcome, QuotaListOutcome::Available);
        assert_eq!(listing.plan_label.as_deref(), Some("Example plan"));
        assert_eq!(listing.snapshots, vec![sample_quota()]);
    }

    #[test]
    fn empty_quota_is_explicit_no_signal() {
        let listing =
            quota_listing_from_results("claude-code", Ok(Vec::new()), Ok(Some("pro".to_string())));
        assert_eq!(listing.outcome, QuotaListOutcome::NoSignal);
        assert_eq!(listing.plan_label.as_deref(), Some("pro"));
        assert!(listing.snapshots.is_empty());
    }

    #[test]
    fn quota_error_is_retained_and_clears_visible_data() {
        let listing = quota_listing_from_results(
            "codex-cli",
            Err(Error::ConfigRead {
                provider: "codex-cli".to_string(),
                reason: "/tmp/cam-test/quota.json is not valid JSON".to_string(),
            }),
            Ok(None),
        );
        assert!(listing.snapshots.is_empty());
        assert_eq!(listing.plan_label, None);
        let QuotaListOutcome::Failed { error } = listing.outcome else {
            panic!("expected failed quota outcome");
        };
        assert_eq!(error.kind, QuotaListErrorKind::ConfigRead);
        assert_eq!(error.path.as_deref(), Some("/tmp/cam-test/quota.json"));
    }

    #[test]
    fn plan_error_is_a_failed_quota_outcome() {
        let listing = quota_listing_from_results(
            "claude-code",
            Ok(Vec::new()),
            Err(Error::ConfigRead {
                provider: "claude-code".to_string(),
                reason: "/tmp/cam-test/.claude.json has malformed plan metadata".to_string(),
            }),
        );
        assert!(listing.snapshots.is_empty());
        assert_eq!(listing.plan_label, None);
        assert!(matches!(
            listing.outcome,
            QuotaListOutcome::Failed {
                error: QuotaListError {
                    kind: QuotaListErrorKind::ConfigRead,
                    ..
                }
            }
        ));
    }

    #[test]
    fn invalid_quota_snapshot_becomes_failed_without_a_number() {
        for utilization in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
            let mut snapshot = sample_quota();
            snapshot.utilization = utilization;
            let listing = quota_listing_from_results("example", Ok(vec![snapshot]), Ok(None));
            assert!(listing.snapshots.is_empty());
            assert!(matches!(
                listing.outcome,
                QuotaListOutcome::Failed {
                    error: QuotaListError {
                        kind: QuotaListErrorKind::InvalidSnapshot,
                        ..
                    }
                }
            ));
        }
    }

    #[test]
    fn invalid_quota_timestamps_become_failed() {
        for (captured_at, resets_at) in [
            ("", None),
            ("not-a-timestamp", None),
            ("2030-01-01T00:00:00Z", Some("tomorrow")),
        ] {
            let mut snapshot = sample_quota();
            snapshot.captured_at = captured_at.to_string();
            snapshot.resets_at = resets_at.map(str::to_string);
            let listing = quota_listing_from_results("example", Ok(vec![snapshot]), Ok(None));
            assert!(listing.snapshots.is_empty());
            assert!(matches!(
                listing.outcome,
                QuotaListOutcome::Failed {
                    error: QuotaListError {
                        kind: QuotaListErrorKind::InvalidSnapshot,
                        ..
                    }
                }
            ));
        }
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
