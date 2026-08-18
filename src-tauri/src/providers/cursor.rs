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
pub struct CursorAdapter;

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
        let Some(home) = home_dir() else {
            return Vec::new();
        };
        vec![
            home.join(".config").join("cursor").join("cli-config.json"),
            home.join(".cursor"),
        ]
    }

    fn detect(&self) -> InstallState {
        let has_config = home_dir()
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
