//! Claude Code (Anthropic) adapter.
//!
//! Observed layout on Linux, Claude Code 2.1.212:
//!
//! - `~/.claude/.credentials.json` [verified-local] — OAuth material under a
//!   `claudeAiOauth` object with the key names `accessToken`, `refreshToken`,
//!   `expiresAt`, `refreshTokenExpiresAt`, `scopes`, `subscriptionType`,
//!   `rateLimitTier`, plus a sibling `organizationUuid`.
//! - `~/.claude.json` [verified-local] — global client state: `oauthAccount`,
//!   `mcpServers`, `projects`, caches, and onboarding flags. Large and rewritten
//!   frequently by the tool.
//! - `~/.claude/settings.json` [verified-local] — user settings.
//! - `~/.claude/` also holds `projects/`, `history.jsonl`, `sessions/`,
//!   `shell-snapshots/`, `plugins/`, and caches [verified-local]. These are
//!   session data, not credentials, and must not be moved by a switch.
//!
//! Switching therefore has to swap `.credentials.json` **and** the identity
//! fields inside `~/.claude.json`; see `docs/research/claude-code.md`.

use std::path::PathBuf;

use super::{binary_on_path, home_dir, ProviderAdapter};
use crate::error::{Error, Result};
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};

#[derive(Debug, Default)]
pub struct ClaudeCodeAdapter;

impl ProviderAdapter for ClaudeCodeAdapter {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            display_name: "Claude Code".to_string(),
            vendor: "Anthropic".to_string(),
            auth_kinds: vec![AuthKind::OAuth, AuthKind::ApiKey],
            maturity: Maturity::Planned,
            install_state: self.detect(),
        }
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let Some(home) = home_dir() else {
            return Vec::new();
        };
        vec![
            home.join(".claude.json"),
            home.join(".claude").join(".credentials.json"),
            home.join(".claude").join("settings.json"),
        ]
    }

    fn detect(&self) -> InstallState {
        let has_config = home_dir()
            .map(|home| home.join(".claude").is_dir())
            .unwrap_or(false);
        if binary_on_path("claude") || has_config {
            InstallState::Installed
        } else {
            InstallState::NotInstalled
        }
    }

    fn list_accounts(&self) -> Result<Vec<Account>> {
        Err(Error::NotImplemented("claude-code::list_accounts"))
    }

    fn activate_account(&self, _account_id: &str) -> Result<()> {
        Err(Error::NotImplemented("claude-code::activate_account"))
    }
}
