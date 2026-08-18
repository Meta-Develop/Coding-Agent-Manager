import { invoke } from '@tauri-apps/api/core'
import type { Account, ProviderDescriptor, QuotaSnapshot } from '@/types'

/**
 * Typed wrappers around the Tauri command surface.
 *
 * Keep this file as the single place the front end reaches into Rust so the
 * command names exist in exactly one location.
 */

export function listProviders(): Promise<ProviderDescriptor[]> {
  return invoke<ProviderDescriptor[]>('list_providers')
}

export function listAccounts(providerId?: string): Promise<Account[]> {
  return invoke<Account[]>('list_accounts', { providerId: providerId ?? null })
}

export function activateAccount(accountId: string): Promise<void> {
  return invoke<void>('activate_account', { accountId })
}

export function listQuota(): Promise<QuotaSnapshot[]> {
  return invoke<QuotaSnapshot[]>('list_quota')
}
