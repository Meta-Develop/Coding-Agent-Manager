//! Grok CLI (xAI) adapter.
//!
//! Observed layout on Linux, grok 0.2.93:
//!
//! - `~/.grok/auth.json` [verified-local] — a map keyed by
//!   `"<oidc-issuer>::<client-uuid>"`. Each entry carries `key`, `auth_mode`,
//!   `create_time`, `user_id`, `email`, `first_name`,
//!   `profile_image_asset_id`, `principal_type`, `principal_id`, `team_id`,
//!   `coding_data_retention_opt_out`, `refresh_token`, `expires_at`,
//!   `oidc_issuer`, and `oidc_client_id`.
//! - `~/.grok/config.toml` [verified-local] — client configuration, including
//!   `[marketplace]` and `[[marketplace.sources]]`.
//! - `~/.grok/auth.json.lock`, `~/.grok/active_sessions.json`,
//!   `~/.grok/active_sessions.lock` [verified-local] — the CLI takes advisory
//!   locks, so any write must respect them rather than clobbering the file.
//! - `~/.grok/models_cache.json` [verified-local] — may expose model
//!   availability; not confirmed to carry quota.
//!
//! Grok is the most promising switching target after Codex: `auth.json` is
//! keyed per identity, so several accounts can coexist and a switch may reduce
//! to selecting the active key rather than swapping whole files. That must be
//! confirmed before it is relied on.

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

#[derive(Debug, Default)]
pub struct GrokCliAdapter;

impl ProviderAdapter for GrokCliAdapter {
    fn id(&self) -> &'static str {
        "grok-cli"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Grok CLI".to_string(),
            vendor: "xAI".to_string(),
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            maturity: Maturity::Planned,
            install_state: self.detect(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(home) = home_dir() else {
            return Vec::new();
        };
        let root = home.join(".grok");
        vec![
            root.join("auth.json"),
            root.join("config.toml"),
            root.join("models_cache.json"),
        ]
    }

    fn detect(&self) -> InstallState {
        let has_config = home_dir()
            .map(|home| home.join(".grok").is_dir())
            .unwrap_or(false);
        if binary_on_path("grok") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        Err(Error::NotImplemented("grok-cli::list_accounts"))
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("grok-cli::activate_account"))
    }
}
