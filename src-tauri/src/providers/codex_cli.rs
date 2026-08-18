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

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

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
            maturity: Maturity::Planned,
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
        Err(Error::NotImplemented("codex-cli::list_accounts"))
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("codex-cli::activate_account"))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;

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
}
