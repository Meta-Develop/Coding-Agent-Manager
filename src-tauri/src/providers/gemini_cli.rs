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
pub struct GeminiCliAdapter;

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
        let Some(home) = home_dir() else {
            return Vec::new();
        };
        let root = home.join(".gemini");
        vec![root.join("projects.json"), root.join("settings.json")]
    }

    fn detect(&self) -> InstallState {
        let has_config = home_dir()
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
