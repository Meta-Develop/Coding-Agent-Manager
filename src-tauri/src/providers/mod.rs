//! Provider adapters.
//!
//! Every managed agent tool is reached through exactly one [`ProviderAdapter`].
//! Core code never special-cases a vendor; adding a tool means adding a module
//! here and one line in [`registry`]. See `docs/ARCHITECTURE.md`.
//!
//! Config-path claims in these modules carry a confidence marker matching
//! `docs/research/`:
//! `[verified-local]` observed on a real installation, `[verified-docs]` from
//! official documentation, `[inferred]` reasoned but unconfirmed,
//! `[unknown]` not yet established. Never upgrade a marker without evidence.

use std::path::PathBuf;

use crate::error::Result;
use crate::model::{Account, InstallState, ProviderDescriptor, QuotaSnapshot};

pub mod claude_code;
pub mod codex_cli;
pub mod cursor;
pub mod gemini_cli;
pub mod grok_cli;

/// The contract every managed tool must satisfy.
///
/// Implementations must be side-effect free except in [`activate_account`],
/// and [`activate_account`] must write a recoverable backup before it replaces
/// any file the user's tool owns (NFR-4 in `docs/SPEC.md`).
pub trait ProviderAdapter: Send + Sync {
    /// Stable identifier used in config, IPC, and on disk. Never renamed.
    fn id(&self) -> &'static str;

    /// Static description of the provider, independent of machine state.
    fn descriptor(&self) -> ProviderDescriptor;

    /// Files and directories this adapter reads or writes on this host.
    ///
    /// Used by diagnostics and by the backup subsystem. Paths that do not exist
    /// are still returned, so the caller can report what was expected.
    fn config_paths(&self) -> Vec<PathBuf>;

    /// Whether the tool appears to be installed on this machine.
    fn detect(&self) -> InstallState;

    /// Accounts the adapter can see, including the currently active one.
    fn list_accounts(&self) -> Result<Vec<Account>>;

    /// Make `account_id` the account the tool will use on its next start.
    ///
    /// Must be atomic from the tool's point of view: either the switch fully
    /// happened or the previous state is intact.
    fn activate_account(&self, account_id: &str) -> Result<()>;

    /// Quota signals the provider exposes, if any.
    ///
    /// Returning an empty vector is correct and expected for providers that
    /// publish no usable signal. Never synthesise a number.
    fn quota(&self) -> Result<Vec<QuotaSnapshot>> {
        Ok(Vec::new())
    }
}

/// All adapters known to this build, in display order.
pub fn registry() -> Vec<Box<dyn ProviderAdapter>> {
    vec![
        Box::new(claude_code::ClaudeCodeAdapter),
        Box::new(codex_cli::CodexCliAdapter),
        Box::new(cursor::CursorAdapter),
        Box::new(grok_cli::GrokCliAdapter),
        Box::new(gemini_cli::GeminiCliAdapter),
    ]
}

/// Look up a single adapter by its stable id.
pub fn find(id: &str) -> Option<Box<dyn ProviderAdapter>> {
    registry().into_iter().find(|adapter| adapter.id() == id)
}

/// Home directory helper shared by adapters.
///
/// Returns `None` rather than panicking so a headless or unusual environment
/// degrades into "not installed" instead of crashing the application.
pub(crate) fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

/// `true` when the named executable resolves on `PATH`.
pub(crate) fn binary_on_path(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(binary).is_file())
}
