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
pub struct GrokCliAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
}

impl GrokCliAdapter {
    /// Root this adapter at `home` instead of the real user home.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
        }
    }

    fn resolved_home(&self) -> Option<PathBuf> {
        self.home.clone().or_else(home_dir)
    }
}

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
        let Some(home) = self.resolved_home() else {
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
        let has_config = self
            .resolved_home()
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

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;

    #[test]
    fn with_home_resolves_config_paths_under_the_injected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = GrokCliAdapter::with_home(dir.path());
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
}
