//! Provider-agnostic domain types.
//!
//! These are mirrored in TypeScript at `src/types/index.ts`. Change both sides
//! together; `docs/ARCHITECTURE.md` describes the contract.

use serde::{Deserialize, Serialize};

/// Stable identifier for a managed agent tool, e.g. `claude-code`.
pub type ProviderId = &'static str;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthKind {
    /// serde kebab-case would emit `"o-auth"`; the webview type is `"oauth"`.
    #[serde(rename = "oauth")]
    OAuth,
    ApiKey,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallState {
    Installed,
    NotInstalled,
    Unknown,
}

/// How complete an adapter is. Surfaced in the UI so users are never misled
/// about what the application can actually do for a given tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    Planned,
    Experimental,
    Supported,
}

/// A mutating account operation an adapter actually implements.
///
/// `maturity` is too coarse for the Accounts page: an experimental adapter
/// may still add, switch, or delete. The UI offers a button only when this
/// list contains the corresponding capability (NFR-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCapability {
    AddAccount,
    SwitchAccount,
    DeleteAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub display_name: String,
    pub vendor: String,
    pub auth_kinds: Vec<AuthKind>,
    pub maturity: Maturity,
    pub install_state: InstallState,
    pub capabilities: Vec<ProviderCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Assigned by this application, never by the vendor.
    pub id: String,
    pub provider_id: String,
    /// User-chosen label. Must never contain a secret.
    pub label: String,
    /// Vendor identity, already masked for display (e.g. `a***@example.com`).
    pub masked_identity: Option<String>,
    pub auth_kind: AuthKind,
    pub is_active: bool,
    /// Whether this application holds a stored copy that
    /// `activate_account` and `delete_account` can act on.
    ///
    /// This is not a validity, currency, or vendor-acceptance signal.
    /// A stored copy may be expired, unused, or rejected by the vendor.
    /// An incomplete row is still stored: `delete_account` can remove
    /// the directory, but `activate_account` cannot use it.
    pub is_stored: bool,
    /// True when this row is a managed directory that does not hold a
    /// usable vendor document (`auth.json` that is a JSON object).
    ///
    /// An incomplete row is an abandoned add, not someone's credentials.
    /// It is listed so the user can delete it rather than inferring that
    /// from a missing identity. It is never `is_active`. Completeness is
    /// structural: this is not a claim that a complete document is
    /// current or accepted by the vendor.
    #[serde(default)]
    pub is_incomplete: bool,
    /// RFC 3339 timestamp, when the adapter can determine one.
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuotaSource {
    LocalFile,
    Api,
    Header,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub account_id: String,
    pub model: Option<String>,
    /// Fraction of the window consumed, 0.0..=1.0, or `None` when the provider
    /// exposes no usable signal. Never fabricate a value here.
    pub utilization: Option<f32>,
    pub resets_at: Option<String>,
    pub captured_at: String,
    pub source: QuotaSource,
}

/// Secret-free relay state exposed to the webview.
///
/// Listener configuration that can contain credentials stays in the Rust core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatus {
    pub running: bool,
    pub bind_address: String,
    pub port: u16,
    pub prefixes: Vec<String>,
}

/// Per-provider result of `list_accounts`.
///
/// A flat `Vec<Account>` cannot tell "listed zero", "cannot list yet", and
/// "the look failed" apart. One struct per adapter keeps a single failure
/// from blanking the other providers, and gives the webview a status it can
/// render without guessing from `maturity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountList {
    pub provider_id: String,
    pub accounts: Vec<Account>,
    pub outcome: AccountListOutcome,
}

/// How `list_accounts` finished for one provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AccountListOutcome {
    /// The adapter enumerated; `accounts` may be empty.
    Listed,
    /// The adapter enumerated the API-key path only and does not inspect
    /// OAuth credentials. `accounts` may still be empty.
    ListedApiKeyOnly,
    /// The adapter enumerated; `accounts` may be empty. Something is
    /// wrong that the user needs to see — typically a damaged live
    /// document — and is described by `error`. This is not a failure
    /// of the look: stored copies are still listed. The error never
    /// contains a credential value (NFR-1).
    ListedWithError { error: AccountListError },
    /// The adapter returned `Error::NotImplemented`.
    NotImplemented,
    /// The adapter is implemented and the look failed.
    Failed { error: AccountListError },
}

/// Kind and, where safe, path of a failed look. Never a credential value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountListError {
    pub kind: AccountListErrorKind,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountListErrorKind {
    ConfigRead,
    CredentialStoreUnavailable,
    Other,
}

// Wire values are shared with `src/types/index.ts`. Change both sides together.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_kind_wire_values() {
        assert_eq!(
            serde_json::to_string(&AuthKind::OAuth).unwrap(),
            r#""oauth""#
        );
        assert_eq!(
            serde_json::to_string(&AuthKind::ApiKey).unwrap(),
            r#""api-key""#
        );
        assert_eq!(
            serde_json::to_string(&AuthKind::Unknown).unwrap(),
            r#""unknown""#
        );
    }

    #[test]
    fn install_state_wire_values() {
        assert_eq!(
            serde_json::to_string(&InstallState::Installed).unwrap(),
            r#""installed""#
        );
        assert_eq!(
            serde_json::to_string(&InstallState::NotInstalled).unwrap(),
            r#""not-installed""#
        );
        assert_eq!(
            serde_json::to_string(&InstallState::Unknown).unwrap(),
            r#""unknown""#
        );
    }

    #[test]
    fn maturity_wire_values() {
        assert_eq!(
            serde_json::to_string(&Maturity::Planned).unwrap(),
            r#""planned""#
        );
        assert_eq!(
            serde_json::to_string(&Maturity::Experimental).unwrap(),
            r#""experimental""#
        );
        assert_eq!(
            serde_json::to_string(&Maturity::Supported).unwrap(),
            r#""supported""#
        );
    }

    #[test]
    fn provider_capability_wire_values() {
        assert_eq!(
            serde_json::to_string(&ProviderCapability::AddAccount).unwrap(),
            r#""add-account""#
        );
        assert_eq!(
            serde_json::to_string(&ProviderCapability::SwitchAccount).unwrap(),
            r#""switch-account""#
        );
        assert_eq!(
            serde_json::to_string(&ProviderCapability::DeleteAccount).unwrap(),
            r#""delete-account""#
        );
    }

    #[test]
    fn provider_descriptor_wire_shape_includes_capabilities() {
        let descriptor = ProviderDescriptor {
            id: "codex-cli".to_string(),
            display_name: "Codex CLI".to_string(),
            vendor: "OpenAI".to_string(),
            auth_kinds: vec![AuthKind::OAuth],
            maturity: Maturity::Experimental,
            install_state: InstallState::Installed,
            capabilities: vec![
                ProviderCapability::AddAccount,
                ProviderCapability::SwitchAccount,
                ProviderCapability::DeleteAccount,
            ],
        };
        assert_eq!(
            serde_json::to_value(&descriptor).unwrap(),
            serde_json::json!({
                "id": "codex-cli",
                "displayName": "Codex CLI",
                "vendor": "OpenAI",
                "authKinds": ["oauth"],
                "maturity": "experimental",
                "installState": "installed",
                "capabilities": ["add-account", "switch-account", "delete-account"]
            })
        );
    }

    #[test]
    fn account_wire_shape_includes_is_stored() {
        let stored = Account {
            id: "acct-work".to_string(),
            provider_id: "codex-cli".to_string(),
            label: "work".to_string(),
            masked_identity: Some("****0001".to_string()),
            auth_kind: AuthKind::OAuth,
            is_active: true,
            is_stored: true,
            is_incomplete: false,
            expires_at: None,
        };
        assert_eq!(
            serde_json::to_value(&stored).unwrap(),
            serde_json::json!({
                "id": "acct-work",
                "providerId": "codex-cli",
                "label": "work",
                "maskedIdentity": "****0001",
                "authKind": "oauth",
                "isActive": true,
                "isStored": true,
                "isIncomplete": false,
                "expiresAt": null
            })
        );

        let live = Account {
            id: "codex-cli-on-disk".to_string(),
            provider_id: "codex-cli".to_string(),
            label: "Codex CLI".to_string(),
            masked_identity: Some("****0001".to_string()),
            auth_kind: AuthKind::OAuth,
            is_active: true,
            is_stored: false,
            is_incomplete: false,
            expires_at: None,
        };
        assert_eq!(
            serde_json::to_value(&live).unwrap(),
            serde_json::json!({
                "id": "codex-cli-on-disk",
                "providerId": "codex-cli",
                "label": "Codex CLI",
                "maskedIdentity": "****0001",
                "authKind": "oauth",
                "isActive": true,
                "isStored": false,
                "isIncomplete": false,
                "expiresAt": null
            })
        );

        let incomplete = Account {
            id: "acct-abandoned".to_string(),
            provider_id: "codex-cli".to_string(),
            label: "acct-abandoned".to_string(),
            masked_identity: None,
            auth_kind: AuthKind::Unknown,
            is_active: false,
            is_stored: true,
            is_incomplete: true,
            expires_at: None,
        };
        assert_eq!(
            serde_json::to_value(&incomplete).unwrap(),
            serde_json::json!({
                "id": "acct-abandoned",
                "providerId": "codex-cli",
                "label": "acct-abandoned",
                "maskedIdentity": null,
                "authKind": "unknown",
                "isActive": false,
                "isStored": true,
                "isIncomplete": true,
                "expiresAt": null
            })
        );
    }

    #[test]
    fn quota_source_wire_values() {
        assert_eq!(
            serde_json::to_string(&QuotaSource::LocalFile).unwrap(),
            r#""local-file""#
        );
        assert_eq!(
            serde_json::to_string(&QuotaSource::Api).unwrap(),
            r#""api""#
        );
        assert_eq!(
            serde_json::to_string(&QuotaSource::Header).unwrap(),
            r#""header""#
        );
        assert_eq!(
            serde_json::to_string(&QuotaSource::Unknown).unwrap(),
            r#""unknown""#
        );
    }

    #[test]
    fn relay_status_wire_shape_contains_only_public_listener_state() {
        let status = RelayStatus {
            running: true,
            bind_address: "127.0.0.1".to_string(),
            port: 8787,
            prefixes: vec![
                "/v1/chat/completions".to_string(),
                "/v1/messages".to_string(),
                "/v1beta/models/*:generateContent".to_string(),
            ],
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "running": true,
                "bindAddress": "127.0.0.1",
                "port": 8787,
                "prefixes": [
                    "/v1/chat/completions",
                    "/v1/messages",
                    "/v1beta/models/*:generateContent"
                ]
            })
        );
    }

    #[test]
    fn account_list_outcome_wire_values() {
        assert_eq!(
            serde_json::to_string(&AccountListOutcome::Listed).unwrap(),
            r#"{"kind":"listed"}"#
        );
        assert_eq!(
            serde_json::to_string(&AccountListOutcome::ListedApiKeyOnly).unwrap(),
            r#"{"kind":"listed-api-key-only"}"#
        );
        let listed_with_error = AccountListOutcome::ListedWithError {
            error: AccountListError {
                kind: AccountListErrorKind::ConfigRead,
                path: Some("/tmp/auth.json".to_string()),
                message: "configuration for `codex-cli` could not be read: /tmp/auth.json is not valid JSON".to_string(),
            },
        };
        assert_eq!(
            serde_json::to_value(&listed_with_error).unwrap(),
            serde_json::json!({
                "kind": "listed-with-error",
                "error": {
                    "kind": "config-read",
                    "path": "/tmp/auth.json",
                    "message": "configuration for `codex-cli` could not be read: /tmp/auth.json is not valid JSON"
                }
            })
        );
        assert_eq!(
            serde_json::to_string(&AccountListOutcome::NotImplemented).unwrap(),
            r#"{"kind":"not-implemented"}"#
        );
        let failed = AccountListOutcome::Failed {
            error: AccountListError {
                kind: AccountListErrorKind::ConfigRead,
                path: Some("/tmp/auth.json".to_string()),
                message: "configuration for `codex-cli` could not be read: /tmp/auth.json is not valid JSON".to_string(),
            },
        };
        assert_eq!(
            serde_json::to_value(&failed).unwrap(),
            serde_json::json!({
                "kind": "failed",
                "error": {
                    "kind": "config-read",
                    "path": "/tmp/auth.json",
                    "message": "configuration for `codex-cli` could not be read: /tmp/auth.json is not valid JSON"
                }
            })
        );
    }

    #[test]
    fn account_list_error_kind_wire_values() {
        assert_eq!(
            serde_json::to_string(&AccountListErrorKind::ConfigRead).unwrap(),
            r#""config-read""#
        );
        assert_eq!(
            serde_json::to_string(&AccountListErrorKind::CredentialStoreUnavailable).unwrap(),
            r#""credential-store-unavailable""#
        );
        assert_eq!(
            serde_json::to_string(&AccountListErrorKind::Other).unwrap(),
            r#""other""#
        );
    }

    #[test]
    fn provider_account_list_wire_shape() {
        let listing = ProviderAccountList {
            provider_id: "codex-cli".to_string(),
            accounts: Vec::new(),
            outcome: AccountListOutcome::Listed,
        };
        assert_eq!(
            serde_json::to_value(&listing).unwrap(),
            serde_json::json!({
                "providerId": "codex-cli",
                "accounts": [],
                "outcome": { "kind": "listed" }
            })
        );
    }
}
