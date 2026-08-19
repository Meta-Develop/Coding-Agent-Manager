//! Cursor adapter (editor and `cursor-agent` CLI).
//!
//! Observed layout on Linux, cursor-agent 2026.06.26:
//!
//! - `~/.config/cursor/cli-config.json` [verified-local] — CLI settings only:
//!   `version`, `editor`, `display`, `permissions`, `approvalMode`, `sandbox`,
//!   `network`, `attribution`. No credential material was present in it.
//! - `~/.cursor/` [verified-local] — `agents/`, `projects/`, `extensions/`,
//!   `skills-cursor/`, `ai-tracking/ai-code-tracking.db`, `argv.json`.
//!
//! The credential location is **`[unknown]`**: nothing credential-shaped was
//! observed in either directory, which suggests the session token lives in the
//! OS keyring or inside the editor's Electron storage.
//!
//! TODO(research): establish where `cursor-agent login` persists its session
//! before writing any switching logic. Until then this adapter must stay
//! read-only. See `docs/research/cursor.md`.

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

#[derive(Debug, Default)]
pub struct CursorAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
}

impl CursorAdapter {
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

impl ProviderAdapter for CursorAdapter {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Cursor".to_string(),
            vendor: "Anysphere".to_string(),
            auth_kinds: vec![AuthKind::Unknown],
            maturity: Maturity::Planned,
            install_state: self.detect(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(home) = self.resolved_home() else {
            return Vec::new();
        };
        vec![
            home.join(".config").join("cursor").join("cli-config.json"),
            home.join(".cursor"),
        ]
    }

    fn detect(&self) -> InstallState {
        let has_config = self
            .resolved_home()
            .map(|home| home.join(".cursor").is_dir())
            .unwrap_or(false);
        if binary_on_path("cursor-agent") || binary_on_path("cursor") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        Err(Error::NotImplemented("cursor::list_accounts"))
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("cursor::activate_account"))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;

    #[test]
    fn with_home_resolves_config_paths_under_the_injected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CursorAdapter::with_home(dir.path());
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
