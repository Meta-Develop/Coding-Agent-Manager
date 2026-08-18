//! Coding Agent Manager — application core.
//!
//! Layering (enforced by review, see `docs/ARCHITECTURE.md`):
//!
//! ```text
//! commands  ->  providers | storage | relay | router
//! providers ->  storage
//! relay     ->  router -> providers
//! ```
//!
//! Nothing below `commands` may depend on Tauri types, so the core stays
//! testable without a webview and reusable from a future headless binary.

pub mod commands;
pub mod error;
pub mod model;
pub mod providers;
pub mod relay;
pub mod router;
pub mod storage;

/// Start the desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::list_accounts,
            commands::activate_account,
            commands::list_quota,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coding Agent Manager");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_exposes_the_five_initial_providers() {
        let ids: Vec<_> = providers::registry()
            .iter()
            .map(|adapter| adapter.id())
            .collect();
        assert_eq!(
            ids,
            vec![
                "claude-code",
                "codex-cli",
                "cursor",
                "grok-cli",
                "gemini-cli"
            ]
        );
    }

    #[test]
    fn every_adapter_id_is_unique() {
        let mut ids: Vec<_> = providers::registry()
            .iter()
            .map(|adapter| adapter.id())
            .collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate provider id in registry()");
    }

    #[test]
    fn find_resolves_a_known_id_and_rejects_an_unknown_one() {
        assert!(providers::find("codex-cli").is_some());
        assert!(providers::find("not-a-provider").is_none());
    }

    #[test]
    fn descriptors_never_claim_more_maturity_than_implemented() {
        // Until an adapter implements list_accounts, it must not advertise
        // itself as `supported`. This test is the guard that keeps the UI
        // honest as adapters land one at a time.
        for adapter in providers::registry() {
            let descriptor = adapter.descriptor();
            if matches!(descriptor.maturity, model::Maturity::Supported) {
                assert!(
                    adapter.list_accounts().is_ok(),
                    "`{}` claims `supported` but cannot list accounts",
                    descriptor.id
                );
            }
        }
    }
}
