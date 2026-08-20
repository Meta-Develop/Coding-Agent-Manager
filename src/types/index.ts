/**
 * Shared domain types.
 *
 * These mirror the Rust types in `src-tauri/src/model.rs`. Any change here must
 * be made on both sides; see `docs/ARCHITECTURE.md` for the contract.
 */

/** Stable identifier for a managed agent tool, e.g. `claude-code`. */
export type ProviderId = string

/** How an account authenticates against its vendor. */
export type AuthKind = 'oauth' | 'api-key' | 'unknown'

/** Whether the local tool for a provider was detected on this machine. */
export type InstallState = 'installed' | 'not-installed' | 'unknown'

/** An account/tool operation an adapter actually implements. */
export type ProviderCapability =
  'add-account' | 'switch-account' | 'delete-account' | 'launch-tool'

export interface ProviderDescriptor {
  id: ProviderId
  displayName: string
  vendor: string
  authKinds: AuthKind[]
  /** Human-readable adapter maturity, surfaced in the Providers page. */
  maturity: 'planned' | 'experimental' | 'supported'
  installState: InstallState
  /**
   * Account/tool operations this adapter will honour. The Accounts page must not offer a
   * button that is missing here (NFR-8). `maturity` does not answer that.
   */
  capabilities: ProviderCapability[]
}

export interface Account {
  /** Stable id assigned by this application, not by the vendor. */
  id: string
  providerId: ProviderId
  /** User-chosen label. Never contains a secret. */
  label: string
  /** Vendor-side identity, redacted for display (e.g. `a***@example.com`). */
  maskedIdentity: string | null
  authKind: AuthKind
  isActive: boolean
  /** Selected only for the next app-owned launch, not globally active. */
  isSelectedForLaunch: boolean
  /**
   * Whether this application owns durable account metadata or material that
   * its core/adapter lifecycle can select or forget. Not a claim that the
   * account is valid, current, or accepted by the vendor. An incomplete row
   * may be forgotten but cannot be selected.
   */
  isStored: boolean
  /**
   * True when provisioning left a structurally incomplete stored account.
   *
   * It is listed so the user can recover or forget it. It is never active or
   * selected for launch. Completeness is structural: not a claim that complete
   * material is current or accepted by the vendor.
   */
  isIncomplete: boolean
  expiresAt: string | null
}

/** Durable lifecycle of application-owned, non-secret account metadata. */
export type StoredAccountState = 'pending' | 'complete' | 'deleting'

/** Non-secret kind of external material owned by a stored account. */
export type StoredAccountMaterial = 'credential-store' | 'vendor-home'

/**
 * Non-secret metadata persisted by the Rust core. Credential references and
 * values and derived paths never cross IPC.
 */
export interface StoredAccountMetadata {
  id: string
  providerId: ProviderId
  label: string
  authKind: AuthKind
  state: StoredAccountState
  material: StoredAccountMaterial
  isSelected: boolean
}

/** Non-secret result of launching a selected provider account. */
export interface LaunchedProcess {
  providerId: ProviderId
  accountId: string
  processId: number
}

export interface QuotaSnapshot {
  accountId: string
  model: string | null
  /** 0..1, or null when the provider exposes no usable signal. */
  utilization: number | null
  resetsAt: string | null
  capturedAt: string
  source: 'local-file' | 'api' | 'header' | 'unknown'
}

/** Secret-free state for the local relay listener. */
export interface RelayStatus {
  running: boolean
  bindAddress: string
  port: number
  /** Inbound dialect paths configured by the Rust relay core. */
  prefixes: string[]
}

/** How `list_accounts` finished for one provider. */
export type AccountListOutcome =
  | { kind: 'listed' }
  | { kind: 'listed-api-key-only' }
  /**
   * The adapter enumerated; `accounts` may be empty. Something is wrong
   * that the user needs to see — typically a damaged live document —
   * and is described by `error`. This is not a failed look: stored
   * copies are still listed. The error never contains a credential
   * value.
   */
  | { kind: 'listed-with-error'; error: AccountListError }
  | { kind: 'not-implemented' }
  | { kind: 'failed'; error: AccountListError }

export type AccountListErrorKind =
  'config-read' | 'credential-store-unavailable' | 'other'

/** Kind and, where safe, path of a failed look. Never a credential value. */
export interface AccountListError {
  kind: AccountListErrorKind
  path: string | null
  message: string
}

/** Per-provider result of `list_accounts`. */
export interface ProviderAccountList {
  providerId: ProviderId
  accounts: Account[]
  outcome: AccountListOutcome
}
