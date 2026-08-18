//! Gemini CLI (Google) adapter.
//!
//! Observed layout on Linux, gemini 0.47.0:
//!
//! - `~/.gemini/projects.json` [verified-local] — a `projects` map; it was
//!   empty on the inspected host.
//!
//! No credential file was present, because the inspected installation had not
//! completed a sign-in. The credential path is therefore `[unknown]` and must
//! be established from a real logged-in installation or from official
//! documentation before any switching logic is written.
//!
//! Gemini CLI additionally supports API-key authentication through the
//! `GEMINI_API_KEY` environment variable [verified-docs], which gives a second,
//! purely environmental switching strategy that touches no files at all.
//!
//! TODO(research): confirm the OAuth credential path and the settings file
//! location. See `docs/research/gemini-cli.md`.

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

#[derive(Debug, Default)]
pub struct GeminiCliAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
}

impl GeminiCliAdapter {
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

impl ProviderAdapter for GeminiCliAdapter {
    fn id(&self) -> &'static str {
        "gemini-cli"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Gemini CLI".to_string(),
            vendor: "Google".to_string(),
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            maturity: Maturity::Planned,
            install_state: self.detect(),
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
        Err(Error::NotImplemented("gemini-cli::list_accounts"))
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("gemini-cli::activate_account"))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;

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
}
