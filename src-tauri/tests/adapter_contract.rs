//! Adapter contract suite (`docs/TESTING.md` §2).
//!
//! Every adapter is driven by the same test body, parameterised over
//! `providers::registry()`. A sixth adapter therefore inherits the suite
//! once it has a `with_home` arm in [`adapter_for_home`]; omitting that arm
//! fails the suite rather than quietly skipping a rule (`FR-4`).
//!
//! Fixture homes are copied into a `TempDir` and injected. No test resolves
//! the real user home: this machine has live `~/.codex` and `~/.grok`
//! directories, and reading them would violate `docs/TESTING.md` §4.

// `common` also holds M1-only helpers (`Fixture`, backup sidecars). They are
// unused here; this crate still shares `tree_digest` so the contract suite
// does not grow a second oracle (`docs/TESTING.md` §2).
#[allow(dead_code)]
mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use coding_agent_manager_lib::error::Error;
use coding_agent_manager_lib::model::{Account, Maturity, ProviderCapability};
use coding_agent_manager_lib::providers::claude_code::ClaudeCodeAdapter;
use coding_agent_manager_lib::providers::codex_cli::CodexCliAdapter;
use coding_agent_manager_lib::providers::cursor::CursorAdapter;
use coding_agent_manager_lib::providers::gemini_cli::GeminiCliAdapter;
use coding_agent_manager_lib::providers::grok_cli::GrokCliAdapter;
use coding_agent_manager_lib::providers::{self, ProviderAdapter};
use tempfile::TempDir;

use crate::common::{assert_no_fake, copy_tree, tree_digest, FAKE_PREFIX};

/// `id()` is stored in application state (`docs/ARCHITECTURE.md` §4) and
/// must stay a kebab-case token. Uniqueness and the exact initial set are
/// already pinned in `lib.rs`; this test asserts the *shape* so a sixth
/// adapter cannot introduce `ClaudeCode` or `codex_cli`. Stability (never
/// renamed) is a process promise and cannot be proved in a single run.
#[test]
fn id_is_kebab_case() {
    for adapter in providers::registry() {
        let id = adapter.id();
        assert!(
            is_kebab_case(id),
            "`{id}` broke id shape: expected kebab-case `[a-z0-9]+(-[a-z0-9]+)*`"
        );
    }
}

/// `NFR-8`: maturity must not overstate what the adapter can do.
///
/// `lib.rs` already refuses `Supported` when `list_accounts` fails. The
/// sharper rule available now is that `Experimental` or better must not
/// return `NotImplemented` from `list_accounts` — that variant means the
/// method is a stub, so advertising anything past `Planned` is a lie.
#[test]
fn maturity_does_not_outpace_list_accounts() {
    for id in registry_ids() {
        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        let maturity = adapter.descriptor().maturity;
        let listed = adapter.list_accounts();

        if matches!(maturity, Maturity::Experimental | Maturity::Supported) {
            assert!(
                !matches!(listed, Err(Error::NotImplemented(_))),
                "`{id}` broke maturity: claims `{maturity:?}` but \
                 `list_accounts` returns NotImplemented"
            );
        }
        if matches!(maturity, Maturity::Supported) {
            assert!(
                listed.is_ok(),
                "`{id}` broke maturity: claims Supported but \
                 `list_accounts` failed: {listed:?}"
            );
        }
    }
}

/// `docs/ARCHITECTURE.md` §4: `config_paths()` feeds the backup subsystem,
/// so every entry must be an absolute path. An adapter that panics when
/// `$HOME` is missing would take the whole process down (`NFR-8`).
///
/// Absolute-ness is asserted directly against injected roots. The
/// never-panic-when-`$HOME`-is-unset half is **unproven**. Adapters resolve
/// their root through `with_home` or `directories::BaseDirs`; Rust runs
/// these tests on parallel threads, so `std::env::set_var` / `remove_var`
/// on `HOME` would race every neighbour. The cases below cover the unusual
/// injected roots that are safe: an empty directory, a name with spaces,
/// and a path that does not exist. The production path
/// (`home` is `None` → `BaseDirs::new()` → `None` when `$HOME` is unset)
/// is not exercised here.
#[test]
fn config_paths_are_absolute_and_tolerate_unusual_homes() {
    let empty = TempDir::new().expect("empty home");
    let spaced = tempfile::Builder::new()
        .prefix("unusual home ")
        .tempdir()
        .expect("home whose name contains spaces");
    let missing = empty.path().join("does-not-exist");
    assert!(
        !missing.exists(),
        "precondition: the missing-home path must not exist"
    );

    let homes: [(&str, &Path); 3] = [
        ("empty directory", empty.path()),
        ("home whose name contains spaces", spaced.path()),
        ("path that does not exist", &missing),
    ];

    for id in registry_ids() {
        for (label, home) in homes {
            let paths = adapter_for_home(id, home).config_paths();
            assert!(
                !paths.is_empty(),
                "`{id}` broke config_paths: returned no paths under {label} {}",
                home.display()
            );
            for path in paths {
                assert!(
                    path.is_absolute(),
                    "`{id}` broke config_paths: {} is not absolute ({label})",
                    path.display()
                );
                assert!(
                    path.starts_with(home),
                    "`{id}` broke config_paths: {} escaped the injected home {} ({label})",
                    path.display(),
                    home.display()
                );
            }
        }
    }
}

/// `docs/ARCHITECTURE.md` §4: `detect()` is cheap and side-effect free.
/// A detector that writes (a cache, a lock, a rewritten settings file)
/// would mutate a user's install the moment the dashboard opens.
#[test]
fn detect_is_side_effect_free() {
    for id in registry_ids() {
        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        let before = tree_digest(home.path());

        let _first = adapter.detect();
        let after_first = tree_digest(home.path());
        assert_eq!(
            before,
            after_first,
            "`{id}` broke detect side-effect freedom: first call mutated \
             the home:\n{}",
            before.diff(&after_first)
        );

        let _second = adapter.detect();
        let after_second = tree_digest(home.path());
        assert_eq!(
            before,
            after_second,
            "`{id}` broke detect side-effect freedom: running it twice \
             mutated the home:\n{}",
            before.diff(&after_second)
        );
    }
}

/// Automated form of `NFR-1`: `list_accounts` returns masked identities,
/// never a value that appears in the fixture's secret material.
///
/// Adapters without a `tests/fixtures/<id>/home` tree have no curated
/// secret material, so the rule is vacuous for them. That fact is
/// printed rather than treated as a silent pass. Adding that directory
/// opts the adapter in; do not `#[ignore]` it.
#[test]
fn list_accounts_never_echoes_fixture_secret_material() {
    let mut exercised = Vec::new();
    let mut vacuous = Vec::new();

    for id in registry_ids() {
        let Some(src) = fixture_home(id) else {
            // Still drive the adapter against an empty home so a stub
            // cannot hide behind "no fixture". There is nothing to
            // compare the result against (`NFR-1`).
            let home = staged_home(id);
            let _ = adapter_for_home(id, home.path()).list_accounts();
            let note = format!("`{id}`: no tests/fixtures/{id}/home; NFR-1 leak check is vacuous");
            eprintln!("adapter_contract {note}");
            vacuous.push(note);
            continue;
        };

        let secrets = fake_tokens_in(&src);
        assert!(
            !secrets.is_empty(),
            "`{id}` broke NFR-1 setup: tests/fixtures/{id}/home contains no \
             `{FAKE_PREFIX}` secret material; the leak check would be vacuous"
        );

        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        match adapter.list_accounts() {
            Ok(accounts) => {
                assert_accounts_hold_no_secret(id, &accounts, &secrets);
                exercised.push(id.to_string());
            }
            Err(Error::NotImplemented(_)) => {
                let note = format!(
                    "`{id}`: fixture exists but list_accounts is NotImplemented; \
                     NFR-1 leak check is vacuous"
                );
                eprintln!("adapter_contract {note}");
                vacuous.push(note);
            }
            Err(error) => {
                panic!("`{id}` broke NFR-1: list_accounts failed on its fixture: {error:?}")
            }
        }
    }

    assert!(
        !exercised.is_empty() || !vacuous.is_empty(),
        "NFR-1 visited no adapter; the registry appears empty"
    );
}

/// `activate_account` is the only mutating method (`docs/ARCHITECTURE.md`
/// §4) and must take a restorable backup first (`NFR-4`).
///
/// No adapter implements it yet. Switching mechanics are `[inferred]` or
/// `[unknown]` in `docs/research/`, and no write path may depend on those
/// markers (`docs/research/README.md`). What is true today: every adapter
/// either implements the method or returns exactly `Error::NotImplemented`,
/// and an unimplemented one writes nothing to the fixture home.
///
/// # NEXT WORKER — backup-ordering assertions (`docs/TESTING.md` §2, `NFR-4`)
///
/// When the first adapter returns anything other than
/// `Error::NotImplemented` from `activate_account`, this test must gain
/// the following assertions in the same change. Do not land the
/// implementation with only the `NotImplemented` arm below still in
/// force; the gap is how a write path ships without a restore check.
///
/// 1. **Backup before the first mutation.** Copy the adapter's fixture
///    `home/` into a `TempDir` and inject it through `with_home`. Record
///    `tree_digest` of that home. Call `activate_account` with an account
///    id that `list_accounts` actually returned (a probe id would make
///    the write path refuse and the check vacuous). After the call:
///    - a restorable backup of every path in `config_paths()` must exist
///      *before* any byte of the fixture home differs from the pre-call
///      digest;
///    - that backup must be visible to `BackupStore::list` and its
///      `manifest.json` must already be on disk (the M1 suite in
///      `m1_exit_criteria.rs` is the oracle for "the backup exists");
///    - the backup root must itself be a `TempDir`. Adapters today have
///      no injected backup root, so the implementation that first writes
///      must accept one — otherwise the snapshot lands in the
///      developer's real application-data directory, which this suite
///      must never touch (`docs/TESTING.md` §4).
///
/// 2. **Forced mid-write failure leaves the fixture restorable.** After
///    the backup exists, force a failure after the first managed write
///    (the public API has no injection hook today; add one, or drive the
///    same `BackupStore::snapshot` / `fsx::write_atomic` /
///    `BackupStore::restore` sequence the adapter itself uses). Then
///    `BackupStore::restore` must return the fixture home to a
///    byte-identical `tree_digest`, including Unix permission bits and
///    paths that did not exist before the switch. Reuse
///    `common::tree_digest`; do not write a second oracle.
///
/// Do not implement either assertion against a guessed write path. Every
/// claim about how a tool selects the active identity is still
/// `[inferred]` or `[unknown]` in `docs/research/` as of this suite.
#[test]
fn activate_account_is_implemented_or_exactly_not_implemented() {
    for id in registry_ids() {
        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        let before = tree_digest(home.path());

        match adapter.activate_account("contract-suite-probe") {
            Err(Error::NotImplemented(_)) => {
                let after = tree_digest(home.path());
                assert_eq!(
                    before,
                    after,
                    "`{id}` broke activate_account: returned NotImplemented \
                     but mutated the home:\n{}",
                    before.diff(&after)
                );
            }
            other => {
                // Implemented. The backup-ordering assertions in the
                // NEXT WORKER comment above are not in force yet; name
                // the adapter so the gap cannot be mistaken for a pass
                // of NFR-4.
                let _ = other;
                eprintln!(
                    "adapter_contract `{id}` implements activate_account; \
                     NFR-4 backup-before-mutation and mid-write restore \
                     are unproven (see NEXT WORKER comment)"
                );
            }
        }
    }
}

/// `NFR-8`: advertised capabilities must match method implementation.
///
/// An adapter that lists a capability must not return `NotImplemented`
/// from the corresponding method, and one that omits it must. The probe
/// id is not a safe path component, so a real implementation refuses
/// before creating a directory, writing the live home, or spawning the
/// vendor CLI. The suite therefore never starts `codex`, never reaches
/// the keychain, and never writes into the real data directory or the
/// real home (`docs/TESTING.md` §4).
#[test]
fn advertised_capabilities_match_method_implementation() {
    const PROBE: &str = "../etc";
    let expected = [
        ProviderCapability::AddAccount,
        ProviderCapability::SwitchAccount,
        ProviderCapability::DeleteAccount,
    ];

    for id in registry_ids() {
        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        let advertised = adapter.descriptor().capabilities;
        let before = tree_digest(home.path());

        for capability in expected {
            let result = match capability {
                ProviderCapability::AddAccount => adapter.add_account(PROBE),
                ProviderCapability::SwitchAccount => adapter.activate_account(PROBE),
                ProviderCapability::DeleteAccount => adapter.delete_account(PROBE),
            };
            let after = tree_digest(home.path());
            assert_eq!(
                before,
                after,
                "`{id}` broke {capability:?}: a path-escape id must not mutate \
                 the home:\n{}",
                before.diff(&after)
            );

            let not_implemented = matches!(result, Err(Error::NotImplemented(_)));
            if advertised.contains(&capability) {
                assert!(
                    !not_implemented,
                    "`{id}` advertises {capability:?}; NotImplemented would \
                     be a lie (got {result:?})"
                );
            } else {
                assert!(
                    not_implemented,
                    "`{id}` does not advertise {capability:?}; returning \
                     {result:?} would overstate what the adapter can do (NFR-8)"
                );
            }
        }
    }
}

/// `NFR-8`: `is_stored` must match whether `delete_account` can act on
/// the listed row.
///
/// Capability is per provider; actionability is per row. An adapter that
/// reports `is_stored: true` for a row whose `delete_account` returns
/// `UnknownAccount` is lying the same way an advertised `NotImplemented`
/// method is. Listed ids are already path-safe. `delete_account` on an
/// unstored row returns `UnknownAccount` (or `NotImplemented`) without
/// creating a directory, writing the live home, or spawning the vendor
/// CLI. A stored row is deleted only under the injected TempDir:
/// `with_home` isolates data under `{home}/.coding-agent-manager`. The
/// suite never starts `codex`, never reaches the keychain, and never
/// writes the real data directory or the real home (`docs/TESTING.md` §4).
#[test]
fn listed_is_stored_matches_delete_account() {
    for id in registry_ids() {
        let home = staged_home(id);
        let adapter = adapter_for_home(id, home.path());
        let before = tree_digest(home.path());
        let accounts = match adapter.list_accounts() {
            Ok(accounts) => accounts,
            Err(Error::NotImplemented(_)) => continue,
            Err(error) => {
                panic!("`{id}` broke is_stored: list_accounts failed on its fixture: {error:?}")
            }
        };

        for account in accounts.iter().filter(|account| !account.is_stored) {
            let result = adapter.delete_account(&account.id);
            let after = tree_digest(home.path());
            assert_eq!(
                before,
                after,
                "`{id}` broke is_stored: deleting unstored `{}` mutated \
                 the home:\n{}",
                account.id,
                before.diff(&after)
            );
            assert!(
                matches!(
                    result,
                    Err(Error::UnknownAccount(_)) | Err(Error::NotImplemented(_))
                ),
                "`{id}` listed `{}` as not stored; delete_account returned \
                 {result:?} instead of UnknownAccount or NotImplemented (NFR-8)",
                account.id
            );
        }

        for account in accounts.iter().filter(|account| account.is_stored) {
            let result = adapter.delete_account(&account.id);
            assert!(
                !matches!(result, Err(Error::UnknownAccount(_))),
                "`{id}` listed `{}` as stored; UnknownAccount would be a lie \
                 (got {result:?})",
                account.id
            );
        }
    }
}

fn registry_ids() -> Vec<&'static str> {
    providers::registry()
        .iter()
        .map(|adapter| adapter.id())
        .collect()
}

/// Construct the adapter `id` rooted at `home`.
///
/// The trait does not expose `with_home`, so each concrete type needs an
/// arm. A registered id without an arm is a failed contract, not a skip:
/// otherwise a sixth adapter could inherit nothing and look green.
fn adapter_for_home(id: &str, home: impl Into<PathBuf>) -> Box<dyn ProviderAdapter> {
    let home = home.into();
    match id {
        "claude-code" => Box::new(ClaudeCodeAdapter::with_home(home)),
        "codex-cli" => Box::new(CodexCliAdapter::with_home(home)),
        "cursor" => Box::new(CursorAdapter::with_home(home)),
        "grok-cli" => Box::new(GrokCliAdapter::with_home(home)),
        "gemini-cli" => Box::new(GeminiCliAdapter::with_home(home)),
        other => panic!(
            "`{other}` is in providers::registry() but adapter_contract.rs \
             has no with_home arm. Add one so the contract suite covers it; \
             a missing arm is a skipped rule."
        ),
    }
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_home(id: &str) -> Option<PathBuf> {
    let path = fixtures_root().join(id).join("home");
    path.is_dir().then_some(path)
}

/// Copy `tests/fixtures/<id>/home` into a `TempDir`, or an empty one when
/// that fixture does not exist. An adapter must behave sanely against a
/// home that contains nothing.
fn staged_home(id: &str) -> TempDir {
    let temp = TempDir::new().unwrap_or_else(|error| {
        panic!("`{id}`: create TempDir: {error}");
    });
    if let Some(src) = fixture_home(id) {
        copy_tree(&src, temp.path());
    }
    temp
}

fn is_kebab_case(id: &str) -> bool {
    if id.is_empty() || id.starts_with('-') || id.ends_with('-') {
        return false;
    }
    id.split('-').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

fn fake_tokens_in(root: &Path) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    collect_fake_tokens(root, &mut tokens);
    tokens
}

fn collect_fake_tokens(path: &Path, out: &mut BTreeSet<String>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            collect_fake_tokens(&entry.path(), out);
        }
        return;
    }
    if !metadata.is_file() {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let mut rest = text.as_str();
    while let Some(index) = rest.find(FAKE_PREFIX) {
        let token = take_fake_token(&rest[index..]);
        out.insert(token.to_string());
        rest = &rest[index + token.len()..];
    }
}

fn take_fake_token(text: &str) -> &str {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .next()
        .unwrap_or(text)
}

fn assert_accounts_hold_no_secret(id: &str, accounts: &[Account], secrets: &BTreeSet<String>) {
    let json = serde_json::to_string(accounts).unwrap_or_else(|error| {
        panic!("`{id}` broke NFR-1: accounts did not serialise: {error}");
    });
    let debug = format!("{accounts:?}");
    assert_no_fake(&format!("`{id}` list_accounts JSON"), &json);
    assert_no_fake(&format!("`{id}` list_accounts Debug"), &debug);

    for account in accounts {
        let fields = [
            ("id", account.id.as_str()),
            ("provider_id", account.provider_id.as_str()),
            ("label", account.label.as_str()),
        ];
        for (field, value) in fields {
            refute_contains_secret(id, field, value, secrets);
        }
        if let Some(identity) = &account.masked_identity {
            refute_contains_secret(id, "masked_identity", identity, secrets);
        }
        if let Some(expires) = &account.expires_at {
            refute_contains_secret(id, "expires_at", expires, secrets);
        }
    }
}

fn refute_contains_secret(id: &str, field: &str, value: &str, secrets: &BTreeSet<String>) {
    for secret in secrets {
        assert!(
            !value.contains(secret),
            "`{id}` broke NFR-1: Account.{field} contains fixture secret material"
        );
    }
}
