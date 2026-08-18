//! Tauri command surface.
//!
//! This is the only place the webview can call into Rust. Keep commands thin:
//! they translate IPC arguments into core calls and translate errors back. No
//! business logic lives here.

use crate::error::Result;
use crate::model::{Account, ProviderDescriptor, QuotaSnapshot};
use crate::providers;

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
/// An adapter that cannot enumerate accounts yet is skipped rather than failing
/// the whole call, so a single unfinished adapter cannot blank the UI.
#[tauri::command]
pub fn list_accounts(provider_id: Option<String>) -> Result<Vec<Account>> {
    let adapters = match provider_id {
        Some(id) => providers::find(&id).into_iter().collect::<Vec<_>>(),
        None => providers::registry(),
    };

    let mut accounts = Vec::new();
    for adapter in adapters {
        if let Ok(found) = adapter.list_accounts() {
            accounts.extend(found);
        }
    }
    Ok(accounts)
}

/// Make an account active in its provider's tool.
#[tauri::command]
pub fn activate_account(account_id: String) -> Result<()> {
    for adapter in providers::registry() {
        if adapter
            .list_accounts()
            .map(|accounts| accounts.iter().any(|account| account.id == account_id))
            .unwrap_or(false)
        {
            return adapter.activate_account(&account_id);
        }
    }
    Err(crate::error::Error::UnknownAccount(account_id))
}

/// Quota snapshots from every adapter that publishes one.
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
