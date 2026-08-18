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

export interface ProviderDescriptor {
  id: ProviderId
  displayName: string
  vendor: string
  authKinds: AuthKind[]
  /** Human-readable adapter maturity, surfaced in the Providers page. */
  maturity: 'planned' | 'experimental' | 'supported'
  installState: InstallState
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
