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

/** A mutating account operation an adapter actually implements. */
export type ProviderCapability =
  'add-account' | 'switch-account' | 'delete-account'

export interface ProviderDescriptor {
  id: ProviderId
  displayName: string
  vendor: string
  authKinds: AuthKind[]
  /** Human-readable adapter maturity, surfaced in the Providers page. */
  maturity: 'planned' | 'experimental' | 'supported'
  installState: InstallState
  /**
   * Operations this adapter will honour. The Accounts page must not offer a
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
  /**
   * Whether this application holds a stored copy that activate and delete
   * can act on. Not a claim that the account is valid, current, or accepted
   * by the vendor. An incomplete row is still stored: delete can remove
   * the directory, but activate cannot use it.
   */
  isStored: boolean
  /**
   * True when this row is a managed directory that does not hold a usable
   * vendor document (`auth.json` that is a JSON object).
   *
   * An incomplete row is an abandoned add, not someone's credentials. It
   * is listed so the user can delete it rather than inferring that from a
   * missing identity. It is never `isActive`. Completeness is structural:
   * not a claim that a complete document is current or accepted by the
   * vendor.
   */
  isIncomplete: boolean
  expiresAt: string | null
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
