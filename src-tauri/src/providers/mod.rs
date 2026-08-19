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

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
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
        Box::new(claude_code::ClaudeCodeAdapter::default()),
        Box::new(codex_cli::CodexCliAdapter::default()),
        Box::new(cursor::CursorAdapter::default()),
        Box::new(grok_cli::GrokCliAdapter::default()),
        Box::new(gemini_cli::GeminiCliAdapter::default()),
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

/// Directory that holds one managed account's vendor-issued files.
///
/// `{data_dir}/accounts/{provider_id}/{account_id}`. Callers pass
/// `paths::project_dirs().data_dir()` in production so the identity triple
/// is never spelled a second time. The live tool home (`~/.codex` and
/// friends) is not one of these directories.
pub(crate) fn managed_account_dir(data_dir: &Path, provider_id: &str, account_id: &str) -> PathBuf {
    data_dir.join("accounts").join(provider_id).join(account_id)
}

/// Application-assigned account ids are path components. Reject anything
/// that could escape the managed-account tree.
pub(crate) fn account_id_is_safe(account_id: &str) -> bool {
    !account_id.is_empty()
        && account_id.len() <= 128
        && !account_id.contains("..")
        && account_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

/// Whether a process whose `comm` or argv0 file name equals `name` appears
/// to be running.
///
/// Detecting by process name is inherently approximate: a renamed binary
/// is a false negative; an unrelated program that reused the name is a
/// false positive. A pid whose `comm` and cmdline are both unreadable is
/// skipped (another false-negative window). When the process table itself
/// cannot be read, this returns `Err` so a writer can refuse rather than
/// guess. Off Linux there is no `/proc` scan, so this always returns `Err`.
pub(crate) fn process_named_is_running(name: &str) -> Result<bool> {
    #[cfg(target_os = "linux")]
    {
        linux_process_named_is_running(name)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "cannot inspect the process table on this platform",
        )))
    }
}

/// `comm` (trimmed) or the argv0 file name equals `name` (or `name.exe`).
///
/// Substring matches are rejected: `codex-helper` is not `codex`. `comm`
/// is Linux's 15-character process name; `codex` fits.
pub(crate) fn process_name_matches(name: &str, comm: &str, argv0: &str) -> bool {
    if comm == name {
        return true;
    }
    let exe = Path::new(argv0)
        .file_name()
        .and_then(|component| component.to_str())
        .unwrap_or("");
    exe == name || exe.strip_suffix(".exe") == Some(name)
}

#[cfg(target_os = "linux")]
fn linux_process_named_is_running(name: &str) -> Result<bool> {
    let proc = Path::new("/proc");
    let entries = std::fs::read_dir(proc).map_err(|error| {
        Error::Io(std::io::Error::new(
            error.kind(),
            format!("reading /proc: {error}"),
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::Io(std::io::Error::new(
                error.kind(),
                format!("reading /proc: {error}"),
            ))
        })?;
        let file_name = entry.file_name();
        let Some(pid) = file_name
            .to_str()
            .filter(|label| !label.is_empty() && label.bytes().all(|byte| byte.is_ascii_digit()))
        else {
            continue;
        };
        let dir = proc.join(pid);
        let comm = std::fs::read_to_string(dir.join("comm")).unwrap_or_default();
        let cmdline = std::fs::read(dir.join("cmdline")).unwrap_or_default();
        if comm.is_empty() && cmdline.is_empty() {
            // Unreadable pid: cannot tell whether it is `name`. Skipping
            // it is a known false-negative window (see the function doc).
            continue;
        }
        let argv0 = cmdline
            .split(|byte| *byte == 0)
            .next()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("");
        if process_name_matches(name, comm.trim(), argv0) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_id_is_safe_rejects_path_escape() {
        assert!(account_id_is_safe("acct-work"));
        assert!(account_id_is_safe("codex-cli-on-disk"));
        assert!(!account_id_is_safe(""));
        assert!(!account_id_is_safe("../etc"));
        assert!(!account_id_is_safe("acct/work"));
        assert!(!account_id_is_safe("acct work"));
    }

    #[test]
    fn managed_account_dir_nests_provider_then_account() {
        let root = Path::new("/data");
        assert_eq!(
            managed_account_dir(root, "codex-cli", "acct-work"),
            PathBuf::from("/data/accounts/codex-cli/acct-work")
        );
    }

    #[test]
    fn process_name_matches_is_exact() {
        assert!(process_name_matches("codex", "codex", ""));
        assert!(process_name_matches("codex", "", "/usr/bin/codex"));
        assert!(process_name_matches("codex", "", "/usr/bin/codex.exe"));
        assert!(!process_name_matches("codex", "codex-helper", ""));
        assert!(!process_name_matches("codex", "", "/usr/bin/codex-helper"));
        assert!(!process_name_matches(
            "codex",
            "cargo",
            "/path/M4-codex-switch/target/debug/deps/foo"
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_named_is_running_is_false_for_an_absent_name() {
        assert!(!process_named_is_running("cam-absent-process-9f3a2c").unwrap());
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn process_named_is_running_cannot_tell_off_linux() {
        assert!(process_named_is_running("codex").is_err());
    }
}
