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

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{
    account_id_is_safe, binary_on_path, home_dir, managed_account_dir, process_named_is_running,
    ProviderAdapter,
};
use crate::backup::{BackupId, BackupStore};
use crate::error::{Error, Result};
use crate::fsx;
use crate::model::{Account, AuthKind, InstallState, Maturity, ProviderDescriptor};
use crate::paths;

/// Application-assigned id for the single on-disk Codex identity.
///
/// Codex stores exactly one identity in one flat document [verified-local], so
/// this names that slot rather than echoing a vendor identifier. SPEC §4
/// assigns `Account.id` here so a vendor changing `tokens.account_id` cannot
/// orphan local state. The same fixture therefore always produces this id.
const ON_DISK_ACCOUNT_ID: &str = "codex-cli-on-disk";

#[derive(Debug, Default)]
pub struct CodexCliAdapter {
    /// Injected home directory. `None` means the real user home, which is
    /// what production uses; tests pass a `tempfile::TempDir` path so no
    /// test can read a developer's real credentials (`docs/TESTING.md` §4).
    home: Option<PathBuf>,
    /// Injected application data directory (per-account homes and backups).
    /// `None` in production means `paths::project_dirs`; tests pass a
    /// `TempDir` so a switch never writes into the developer's real
    /// application-data directory (`docs/TESTING.md` §4).
    data_dir: Option<PathBuf>,
    /// Override for the running-tool check.
    ///
    /// `None` means inspect the host process table, except when `home` is
    /// injected — a fixture home is not the host's Codex, so the host
    /// process table is ignored. `Some(Ok(b))` forces the answer.
    /// `Some(Err(()))` forces the cannot-tell path, which must refuse.
    injected_tool_running: Option<std::result::Result<bool, ()>>,
    #[cfg(test)]
    fault: SwitchFault,
}

/// Test-only injection that fires during `activate_account`.
///
/// Production builds do not carry this field. The public API has no fault
/// hook; unit tests use it to pin backup-before-write and restore-on-failure
/// (`docs/TESTING.md` §2, `NFR-4`).
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum SwitchFault {
    #[default]
    None,
    AfterSnapshot,
    AfterWrite,
}

impl CodexCliAdapter {
    /// Root this adapter at `home` instead of the real user home.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
            ..Self::default()
        }
    }

    /// Root per-account homes and backups at `data_dir`.
    ///
    /// Production leaves this unset and uses `paths::project_dirs`. Tests
    /// pass a `TempDir` so the snapshot never lands in the developer's
    /// real application-data directory.
    pub fn with_data_dir(mut self, data_dir: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    /// Override the running-tool check.
    ///
    /// Detecting Codex by process name is approximate (see
    /// [`Self::tool_is_running`]). Tests force the answer so the host's
    /// `codex app-server` cannot make a fixture switch refuse, and so the
    /// refusal path can be exercised without spawning a real Codex.
    pub fn with_tool_running(mut self, running: bool) -> Self {
        self.injected_tool_running = Some(Ok(running));
        self
    }

    #[cfg(test)]
    fn with_tool_undetermined(mut self) -> Self {
        self.injected_tool_running = Some(Err(()));
        self
    }

    #[cfg(test)]
    fn with_fault(mut self, fault: SwitchFault) -> Self {
        self.fault = fault;
        self
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

    fn config_read(&self, reason: impl Into<String>) -> Error {
        Error::ConfigRead {
            provider: self.id().to_string(),
            reason: reason.into(),
        }
    }

    fn config_write(&self, reason: impl Into<String>) -> Error {
        Error::ConfigWrite {
            provider: self.id().to_string(),
            reason: reason.into(),
        }
    }

    /// Application data directory: injected, else isolated under an
    /// injected home, else `paths::project_dirs`.
    ///
    /// The identity triple is spelled only in `paths::project_dirs`. An
    /// injected home without an injected data dir falls back to
    /// `{home}/.coding-agent-manager` so `with_home` (used by the contract
    /// suite) cannot write into the developer's real data directory.
    fn resolved_data_dir(&self) -> Option<PathBuf> {
        if let Some(dir) = &self.data_dir {
            return Some(dir.clone());
        }
        if let Some(home) = &self.home {
            return Some(home.join(".coding-agent-manager"));
        }
        paths::project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
    }

    fn backup_store(&self) -> Result<BackupStore> {
        let Some(data_dir) = self.resolved_data_dir() else {
            return Err(
                self.config_write("application data directory is unavailable; no backup root")
            );
        };
        Ok(BackupStore::new(data_dir.join("backups")))
    }

    fn account_auth_path(&self, account_id: &str) -> Result<PathBuf> {
        if !account_id_is_safe(account_id) {
            return Err(Error::UnknownAccount(account_id.to_string()));
        }
        let Some(data_dir) = self.resolved_data_dir() else {
            return Err(self.config_write(
                "application data directory is unavailable; cannot locate the account",
            ));
        };
        Ok(managed_account_dir(&data_dir, self.id(), account_id).join("auth.json"))
    }

    /// Whether Codex appears to be running.
    ///
    /// Detecting by process name is inherently approximate: a renamed
    /// binary is missed; an unrelated program named `codex` is a false
    /// hit; a pid we cannot inspect is skipped. The check is conservative
    /// about *writes*: when the process table cannot be read, this
    /// returns `Err` and the caller refuses rather than replace
    /// `auth.json` under a possibly-live process (`NFR-4`,
    /// `docs/ARCHITECTURE.md` §8). On this host a VS Code extension
    /// keeps `codex app-server` alive continuously, so "the tool is
    /// running" is the normal state.
    ///
    /// An injected home is a fixture, not the host's Codex: the host
    /// process table is ignored unless `with_tool_running` set an
    /// override. Integration tests compile this crate without
    /// `cfg(test)`, so that skip cannot live only behind `cfg(test)`.
    fn tool_is_running(&self) -> Result<bool> {
        match self.injected_tool_running {
            Some(Ok(running)) => return Ok(running),
            Some(Err(())) => {
                return Err(Error::Io(std::io::Error::other(
                    "cannot inspect the process table",
                )));
            }
            None => {}
        }
        if self.home.is_some() {
            return Ok(false);
        }
        process_named_is_running("codex")
    }

    /// Load the vendor-issued document for `account_id` without inspecting
    /// secret fields. Structure only: the file exists, is a regular file,
    /// and is a JSON object. Bytes are returned so the switch can write
    /// them through `fsx::write_atomic` without parsing tokens.
    fn load_account_auth(&self, account_id: &str) -> Result<(PathBuf, Vec<u8>)> {
        let path = self.account_auth_path(account_id)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(Error::UnknownAccount(account_id.to_string()));
            }
            Err(error) => {
                return Err(self.config_read(format!("{} ({})", path.display(), error.kind())));
            }
        };
        if !metadata.is_file() {
            return Err(self.config_read(format!("{} is not a regular file", path.display())));
        }
        let bytes = fs::read(&path)
            .map_err(|error| self.config_read(format!("{} ({})", path.display(), error.kind())))?;
        // serde's error text can echo a token; never include it (NFR-1).
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| self.config_read(format!("{} is not valid JSON", path.display())))?;
        if !value.is_object() {
            return Err(self.config_read(format!("{} is not a JSON object", path.display())));
        }
        Ok((path, bytes))
    }

    /// Confirm the live `auth.json` is the document we just wrote.
    ///
    /// What this proves: the live home now holds the same bytes as the
    /// per-account file, they form a JSON object, and a reader of that
    /// path sees the whole new document (`fsx::write_atomic`).
    ///
    /// What this does not prove: that `codex login status` would report
    /// the expected identity; that the copied credential still works
    /// against the vendor; that a long-running Codex process will pick
    /// the file up; that the server-side session remains valid. The
    /// 2026-08-19 probe established only that `auth.json` determines
    /// local `login status` for a given `CODEX_HOME` [verified-local],
    /// and explicitly not vendor acceptance (`docs/research/codex-cli.md`
    /// §5). Invoking `codex login status` would add a subprocess and
    /// still only report what the CLI believes. No stronger check is
    /// possible without a network call, which a local switch must not
    /// require (`NFR-3`). Step 4 is therefore this file-level check,
    /// not a CLI invocation.
    fn verify_live_auth(&self, dest: &Path, expected: &[u8]) -> Result<()> {
        let got = fs::read(dest).map_err(|error| {
            self.config_write(format!(
                "could not re-read {} after write ({})",
                dest.display(),
                error.kind()
            ))
        })?;
        if got.as_slice() != expected {
            return Err(self.config_write(format!(
                "{} did not match the account document after write",
                dest.display()
            )));
        }
        let value: serde_json::Value = serde_json::from_slice(&got).map_err(|_| {
            self.config_write(format!("{} is not valid JSON after write", dest.display()))
        })?;
        if !value.is_object() {
            return Err(self.config_write(format!(
                "{} is not a JSON object after write",
                dest.display()
            )));
        }
        Ok(())
    }

    /// Restore `backup` and return a write error that says the previous
    /// account is still active. Never includes file contents (NFR-1).
    fn abort_switch(
        &self,
        store: &BackupStore,
        backup: &BackupId,
        reason: impl Into<String>,
    ) -> Error {
        let reason = reason.into();
        match store.restore(backup) {
            Ok(()) => self.config_write(format!("{reason}; the previous account is still active")),
            Err(restore_error) => self.config_write(format!(
                "{reason}; restore of the previous account also failed: {restore_error}"
            )),
        }
    }
}

/// Mask a vendor identifier for display.
///
/// Keeps the last four characters and replaces the rest with a fixed `****`
/// prefix (e.g. `****ab12`). Returns `None` when the value is absent-as-empty
/// or too short to mask without leaving the original essentially intact.
///
/// The only identity-shaped field in `auth.json` is `tokens.account_id`
/// [verified-local]. Callers must pass that field and never a token or key.
fn mask_identity(raw: &str) -> Option<String> {
    const VISIBLE: usize = 4;
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= VISIBLE {
        return None;
    }
    let tail: String = chars[chars.len() - VISIBLE..].iter().collect();
    Some(format!("****{tail}"))
}

/// Build the single `Account` that a parsed `auth.json` object represents.
///
/// The document holds exactly one identity [verified-local], so whatever is in
/// it is what the tool will use — `is_active` is therefore always `true`.
///
/// Classification inspects structure only (NFR-1 / threat T2):
/// - `ApiKey` when `OPENAI_API_KEY` is a non-null string [verified-local]
/// - otherwise `OAuth` when a `tokens` object is present [verified-local]
/// - otherwise `AuthKind::Unknown`
///
/// Token, key, and raw `account_id` values are never copied onto the `Account`.
/// `id_token` is a JWT and is not decoded.
fn account_from_auth(
    provider_id: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Account {
    let has_api_key = matches!(
        object.get("OPENAI_API_KEY"),
        Some(serde_json::Value::String(_))
    );
    let tokens = object.get("tokens").and_then(serde_json::Value::as_object);
    let auth_kind = if has_api_key {
        AuthKind::ApiKey
    } else if tokens.is_some() {
        AuthKind::OAuth
    } else {
        AuthKind::Unknown
    };
    let masked_identity = tokens
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(mask_identity);

    Account {
        id: ON_DISK_ACCOUNT_ID.to_string(),
        provider_id: provider_id.to_string(),
        label: "Codex CLI".to_string(),
        masked_identity,
        auth_kind,
        is_active: true,
        // `last_refresh` is a refresh timestamp, not an expiry [verified-local].
        // Inventing `expires_at` from it would fabricate a capability the file
        // does not provide (NFR-8).
        expires_at: None,
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
            // Switching writes a vendor-issued `auth.json` into the live
            // home. That does not prove the copied credential works against
            // the vendor (`docs/research/codex-cli.md` §5), so `Supported`
            // would overstate what this adapter can do (NFR-8).
            maturity: Maturity::Experimental,
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
        let Some(path) = self.codex_home().map(|root| root.join("auth.json")) else {
            return Ok(Vec::new());
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            // A missing file means no account is configured [verified-local].
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(self.config_read(format!("{} ({})", path.display(), error.kind())));
            }
        };
        // Reason strings name the path and the kind of failure only. The serde
        // error text can echo a token from the file; never include it (NFR-1).
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| self.config_read(format!("{} is not valid JSON", path.display())))?;
        let Some(object) = value.as_object() else {
            return Err(self.config_read(format!("{} is not a JSON object", path.display())));
        };
        Ok(vec![account_from_auth(self.id(), object)])
    }

    fn activate_account(&self, account_id: &str) -> Result<()> {
        // `docs/ARCHITECTURE.md` §5, in order. The document being written
        // is one the tool itself issued into a per-account directory; this
        // application does not invent credential JSON. Replacing auth.json
        // in the resolved Codex home is the local identity switch
        // `login status` follows: a 2026-08-19 probe on a full copy of a
        // populated live home showed that file alone decides the reported
        // identity [verified-local] (`docs/research/codex-cli.md` §5).
        // Vendor acceptance of a moved credential is untested; `login
        // status` is not a model request.

        // 1. Refuse if the tool is running. Error has no ToolRunning
        // variant; ConfigWrite is the closest existing one (we are
        // refusing a write). A dedicated variant would be better so the
        // UI can render a distinct "close the tool" recovery path.
        match self.tool_is_running() {
            Ok(true) => {
                return Err(self.config_write(
                    "Codex CLI appears to be running (process name `codex`). \
                     Close the Codex CLI and any VS Code Codex extension \
                     (`codex app-server`) before switching. Replacing \
                     auth.json while the tool is running can corrupt the \
                     credential file",
                ));
            }
            Ok(false) => {}
            Err(_) => {
                return Err(self.config_write(
                    "could not determine whether Codex CLI is running; \
                     refusing to replace auth.json. Close the Codex CLI \
                     and any VS Code Codex extension (`codex app-server`), \
                     then retry",
                ));
            }
        }

        // Structure-only read of the per-account file (exists, regular
        // file, JSON object). Unknown accounts must not snapshot or
        // write. This is not a CredentialStore lookup: we move a file.
        let (_source, bytes) = self.load_account_auth(account_id)?;

        let Some(live_auth) = self.codex_home().map(|root| root.join("auth.json")) else {
            return Err(self.config_write("Codex home directory is unavailable"));
        };

        // 2. Snapshot every path in config_paths() before anything is
        // written anywhere.
        let store = self.backup_store()?;
        let backup = store.snapshot(self.id(), &self.config_paths())?;

        let fail = |reason: String| Err(self.abort_switch(&store, &backup, reason));

        #[cfg(test)]
        if self.fault == SwitchFault::AfterSnapshot {
            return fail("injected failure after snapshot".to_string());
        }

        // 3. Write the target document into the live home atomically.
        // config.toml and every other live-home file are left untouched.
        if let Some(parent) = live_auth.parent() {
            if let Err(error) = fsx::create_dir_all_private(parent) {
                return fail(format!("could not create {}: {error}", parent.display()));
            }
        }
        if let Err(error) = fsx::write_atomic(&live_auth, &bytes) {
            return fail(format!("could not write {}: {error}", live_auth.display()));
        }

        #[cfg(test)]
        if self.fault == SwitchFault::AfterWrite {
            return fail("injected failure after write".to_string());
        }

        // 4. Verify the live file is the document we intended. See
        // `verify_live_auth` for what this does and does not prove.
        if let Err(error) = self.verify_live_auth(&live_auth, &bytes) {
            let reason = match error {
                Error::ConfigWrite { reason, .. } => reason,
                other => other.to_string(),
            };
            return fail(reason);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAdapter;
    use super::*;
    use std::fs;
    use std::path::Path;

    const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-cli");
    const FAKE_PREFIX: &str = "FAKE-";

    fn staged_home(name: &str) -> tempfile::TempDir {
        let src = Path::new(FIXTURE_ROOT).join(name);
        let temp = tempfile::tempdir().expect("tempdir");
        copy_tree(&src, temp.path());
        temp
    }

    fn copy_tree(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap_or_else(|error| {
            panic!("create {}: {error}", dst.display());
        });
        for entry in fs::read_dir(src).unwrap_or_else(|error| {
            panic!("read_dir {}: {error}", src.display());
        }) {
            let entry = entry.expect("dirent");
            let to = dst.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_tree(&entry.path(), &to);
            } else {
                fs::copy(entry.path(), &to).unwrap_or_else(|error| {
                    panic!(
                        "copy {} -> {}: {error}",
                        entry.path().display(),
                        to.display()
                    );
                });
            }
        }
    }

    fn assert_no_fake(where_: &str, text: &str) {
        assert!(
            !text.contains(FAKE_PREFIX),
            "{where_} leaked fixture secret material: {text}"
        );
    }

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

    #[test]
    fn mask_identity_keeps_only_the_last_four_characters() {
        assert_eq!(mask_identity("ab12cd34"), Some("****cd34".to_string()));
        assert_eq!(mask_identity("wxyz"), None);
        assert_eq!(mask_identity("abc"), None);
        assert_eq!(mask_identity(""), None);
    }

    #[test]
    fn list_accounts_matches_the_oauth_fixture_expectation() {
        let home = staged_home("home");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let expected_path = Path::new(FIXTURE_ROOT).join("expected/accounts.json");
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&expected_path).expect("expected/accounts.json"),
        )
        .expect("expected json");
        let got = serde_json::to_value(&accounts).expect("serialize");
        assert_eq!(got, expected);

        assert_eq!(accounts.len(), 1);
        assert!(accounts[0].is_active);
        assert_eq!(accounts[0].auth_kind, AuthKind::OAuth);
        assert_eq!(accounts[0].masked_identity.as_deref(), Some("****0001"));
        assert_eq!(accounts[0].expires_at, None);
        assert_eq!(accounts[0].id, ON_DISK_ACCOUNT_ID);

        let again = adapter.list_accounts().expect("second list");
        assert_eq!(again[0].id, accounts[0].id);
    }

    #[test]
    fn list_accounts_classifies_a_non_null_api_key_as_api_key() {
        let home = staged_home("home-api-key");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].auth_kind, AuthKind::ApiKey);
        assert!(accounts[0].is_active);
        assert_eq!(accounts[0].expires_at, None);
        assert_eq!(accounts[0].masked_identity, None);
        assert_no_fake(
            "api-key list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
    }

    #[test]
    fn list_accounts_returns_empty_when_auth_json_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CodexCliAdapter::with_home(dir.path());
        let accounts = adapter
            .list_accounts()
            .expect("a missing auth.json is not an error");
        assert!(accounts.is_empty());
    }

    #[test]
    fn list_accounts_rejects_malformed_auth_json() {
        // Written here rather than stored as a `.json` fixture: Prettier parses
        // tracked `*.json` and refuses this intentionally invalid document.
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".codex");
        fs::create_dir_all(&auth_dir).expect("mkdir .codex");
        fs::write(
            auth_dir.join("auth.json"),
            r#"{this is not json, "OPENAI_API_KEY": "FAKE-malformed-key-0001""#,
        )
        .expect("write malformed auth.json");
        let adapter = CodexCliAdapter::with_home(home.path());
        let error = adapter
            .list_accounts()
            .expect_err("unparsable auth.json must not be guessed at");
        assert!(
            matches!(error, Error::ConfigRead { .. }),
            "expected ConfigRead, got {error:?}"
        );
        assert_no_fake("ConfigRead Display", &error.to_string());
        assert_no_fake("ConfigRead Debug", &format!("{error:?}"));
    }

    #[test]
    fn list_accounts_never_returns_fixture_secret_material() {
        let home = staged_home("home");
        let adapter = CodexCliAdapter::with_home(home.path());
        let accounts = adapter.list_accounts().expect("list");

        let auth = fs::read_to_string(home.path().join(".codex/auth.json")).expect("auth.json");
        assert!(
            auth.contains(FAKE_PREFIX),
            "fixture lost its {FAKE_PREFIX} values; the leak grep would be vacuous"
        );

        assert_no_fake(
            "list_accounts json",
            &serde_json::to_string(&accounts).expect("json"),
        );
        assert_no_fake("list_accounts debug", &format!("{accounts:?}"));
        for account in &accounts {
            assert_no_fake("account.id", &account.id);
            assert_no_fake("account.label", &account.label);
            if let Some(identity) = &account.masked_identity {
                assert_no_fake("account.masked_identity", identity);
            }
        }
    }

    #[test]
    fn descriptor_stays_experimental_after_switch_is_implemented() {
        let dir = tempfile::tempdir().expect("tempdir");
        let adapter = CodexCliAdapter::with_home(dir.path());
        assert_eq!(adapter.descriptor().maturity, Maturity::Experimental);
        assert!(
            !matches!(
                adapter.activate_account("codex-cli-on-disk"),
                Err(Error::NotImplemented(_))
            ),
            "activate_account is implemented; NotImplemented would be a lie"
        );
    }

    const TARGET_ACCOUNT: &str = "acct-work";
    const TARGET_AUTH: &str = r#"{
  "auth_mode": "plan",
  "OPENAI_API_KEY": null,
  "tokens": {
    "id_token": "FAKE-id-token-0002",
    "access_token": "FAKE-access-token-0002",
    "refresh_token": "FAKE-refresh-token-0002",
    "account_id": "FAKE-account-0002"
  },
  "last_refresh": "2026-08-18T00:00:00.000Z"
}
"#;

    struct SwitchEnv {
        live: tempfile::TempDir,
        data: tempfile::TempDir,
    }

    impl SwitchEnv {
        fn new() -> Self {
            let live = staged_home("home");
            let data = tempfile::tempdir().expect("data dir");
            let account_dir = data.path().join("accounts/codex-cli").join(TARGET_ACCOUNT);
            fs::create_dir_all(&account_dir).expect("account dir");
            fs::write(account_dir.join("auth.json"), TARGET_AUTH).expect("target auth");
            Self { live, data }
        }

        fn adapter(&self) -> CodexCliAdapter {
            CodexCliAdapter::with_home(self.live.path())
                .with_data_dir(self.data.path())
                .with_tool_running(false)
        }

        fn backups(&self) -> BackupStore {
            BackupStore::new(self.data.path().join("backups"))
        }

        fn digest(&self) -> std::collections::BTreeMap<String, Vec<u8>> {
            digest_codex(self.live.path())
        }
    }

    fn digest_codex(home: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        let root = home.join(".codex");
        let mut out = std::collections::BTreeMap::new();
        collect_files(&root, &root, &mut out);
        out
    }

    fn collect_files(
        root: &Path,
        path: &Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        let metadata = fs::symlink_metadata(path).unwrap_or_else(|error| {
            panic!("digest metadata for {}: {error}", path.display());
        });
        if metadata.is_file() {
            let rel = path.strip_prefix(root).expect("path under .codex");
            let key = rel
                .iter()
                .map(|component| component.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.insert(key, fs::read(path).expect("digest read"));
        } else if metadata.is_dir() {
            for entry in fs::read_dir(path).expect("digest read_dir") {
                collect_files(root, &entry.expect("dirent").path(), out);
            }
        }
    }

    fn files_except<'a>(
        tree: &'a std::collections::BTreeMap<String, Vec<u8>>,
        skip: &str,
    ) -> std::collections::BTreeMap<&'a str, &'a [u8]> {
        tree.iter()
            .filter(|(path, _)| path.as_str() != skip)
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
            .collect()
    }

    fn digest_brief(tree: &std::collections::BTreeMap<String, Vec<u8>>) -> String {
        tree.iter()
            .map(|(path, bytes)| format!("{path} {}b", bytes.len()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn assert_switch_error_holds_no_secret(error: &Error) {
        assert_no_fake("activate_account Display", &error.to_string());
        assert_no_fake("activate_account Debug", &format!("{error:?}"));
    }

    #[test]
    fn activate_account_replaces_auth_json_and_leaves_every_other_file_byte_identical() {
        let env = SwitchEnv::new();
        let before = env.digest();
        assert_ne!(
            before.get("auth.json").map(Vec::as_slice),
            Some(TARGET_AUTH.as_bytes()),
            "precondition: live auth.json must differ from the target"
        );
        assert!(
            before.contains_key("config.toml"),
            "precondition: live home must have config.toml"
        );
        assert!(
            before.contains_key("sessions/history.jsonl"),
            "precondition: live home must have a sibling the switch must not touch"
        );

        env.adapter()
            .activate_account(TARGET_ACCOUNT)
            .expect("switch");

        let after = env.digest();
        assert_eq!(
            after.get("auth.json").map(Vec::as_slice),
            Some(TARGET_AUTH.as_bytes()),
            "live auth.json must be the target document"
        );
        assert_eq!(
            files_except(&before, "auth.json"),
            files_except(&after, "auth.json"),
            "every other live-home file must be byte-identical; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );
    }

    #[test]
    fn activate_account_writes_a_backup_before_the_first_mutation() {
        let env = SwitchEnv::new();
        let before = env.digest();

        let error = env
            .adapter()
            .with_fault(SwitchFault::AfterSnapshot)
            .activate_account(TARGET_ACCOUNT)
            .expect_err("injected failure after snapshot");
        assert!(
            matches!(error, Error::ConfigWrite { .. }),
            "expected ConfigWrite, got {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("previous account is still active"),
            "unexpected error: {error}"
        );
        assert_switch_error_holds_no_secret(&error);

        let after = env.digest();
        assert_eq!(
            before,
            after,
            "a failure after snapshot must not mutate the live home; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );

        let listed = env.backups().list().expect("list backups");
        assert_eq!(
            listed.len(),
            1,
            "a restorable backup must exist before the first write"
        );
        assert_eq!(listed[0].provider_id, "codex-cli");
        assert!(
            env.data
                .path()
                .join("backups")
                .join(listed[0].id.as_str())
                .join("manifest.json")
                .is_file(),
            "backup manifest must already be on disk"
        );
    }

    #[test]
    fn activate_account_restores_the_live_home_after_a_forced_write_failure() {
        let env = SwitchEnv::new();
        let before = env.digest();

        let error = env
            .adapter()
            .with_fault(SwitchFault::AfterWrite)
            .activate_account(TARGET_ACCOUNT)
            .expect_err("injected failure after write");
        assert!(
            matches!(error, Error::ConfigWrite { .. }),
            "expected ConfigWrite, got {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("previous account is still active"),
            "unexpected error: {error}"
        );
        assert_switch_error_holds_no_secret(&error);

        let after = env.digest();
        assert_eq!(
            before,
            after,
            "restore must return the live home to a byte-identical state; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );
    }

    #[test]
    fn activate_account_rejects_an_unknown_account_without_touching_anything() {
        let env = SwitchEnv::new();
        let before = env.digest();

        let error = env
            .adapter()
            .activate_account("no-such-account")
            .expect_err("unknown account");
        assert!(
            matches!(error, Error::UnknownAccount(ref id) if id == "no-such-account"),
            "expected UnknownAccount, got {error:?}"
        );
        assert_switch_error_holds_no_secret(&error);

        let after = env.digest();
        assert_eq!(
            before,
            after,
            "unknown account must not mutate the live home; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );
        assert!(
            env.backups().list().expect("list").is_empty(),
            "unknown account must not write a backup"
        );
        assert!(
            !env.data.path().join("backups").exists(),
            "unknown account must not create the backup root"
        );
    }

    #[test]
    fn activate_account_rejects_a_path_escape_account_id_without_touching_anything() {
        let env = SwitchEnv::new();
        let before = env.digest();
        let error = env
            .adapter()
            .activate_account("../etc")
            .expect_err("escaped id");
        assert!(
            matches!(error, Error::UnknownAccount(ref id) if id == "../etc"),
            "expected UnknownAccount, got {error:?}"
        );
        assert_eq!(before, env.digest());
        assert!(!env.data.path().join("backups").exists());
    }

    #[test]
    fn activate_account_refuses_while_the_tool_is_running_and_writes_nothing() {
        let env = SwitchEnv::new();
        let before = env.digest();

        let error = env
            .adapter()
            .with_tool_running(true)
            .activate_account(TARGET_ACCOUNT)
            .expect_err("running tool");
        assert!(
            matches!(error, Error::ConfigWrite { .. }),
            "expected ConfigWrite, got {error:?}"
        );
        assert!(
            error.to_string().contains("appears to be running"),
            "user must be told what is running: {error}"
        );
        assert!(
            error.to_string().contains("codex app-server"),
            "user must be told what to close: {error}"
        );
        assert_switch_error_holds_no_secret(&error);

        let after = env.digest();
        assert_eq!(
            before,
            after,
            "running-tool refusal must write nothing; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );
        assert!(
            !env.data.path().join("backups").exists(),
            "running-tool refusal must not snapshot"
        );
    }

    #[test]
    fn activate_account_refuses_when_it_cannot_tell_whether_the_tool_is_running() {
        let env = SwitchEnv::new();
        let before = env.digest();

        let error = env
            .adapter()
            .with_tool_undetermined()
            .activate_account(TARGET_ACCOUNT)
            .expect_err("cannot tell");
        assert!(
            matches!(error, Error::ConfigWrite { .. }),
            "expected ConfigWrite, got {error:?}"
        );
        assert!(
            error.to_string().contains("could not determine"),
            "user must be told the check failed: {error}"
        );
        assert_switch_error_holds_no_secret(&error);

        let after = env.digest();
        assert_eq!(
            before,
            after,
            "cannot-tell must write nothing; before={} after={}",
            digest_brief(&before),
            digest_brief(&after)
        );
        assert!(
            !env.data.path().join("backups").exists(),
            "cannot-tell must not snapshot"
        );
    }
}
