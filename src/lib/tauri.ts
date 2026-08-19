import { invoke } from '@tauri-apps/api/core'
import type {
  ProviderAccountList,
  ProviderDescriptor,
  QuotaSnapshot,
} from '@/types'

/**
 * Typed wrappers around the Tauri command surface.
 *
 * Keep this file as the single place the front end reaches into Rust so the
 * command names exist in exactly one location.
 */

export function listProviders(): Promise<ProviderDescriptor[]> {
  return invoke<ProviderDescriptor[]>('list_providers')
}

export function listAccounts(
  providerId?: string,
): Promise<ProviderAccountList[]> {
  return invoke<ProviderAccountList[]>('list_accounts', {
    providerId: providerId ?? null,
  })
}

export function addAccount(
  providerId: string,
  accountId: string,
): Promise<void> {
  return invoke<void>('add_account', { providerId, accountId })
}

export function activateAccount(
  providerId: string,
  accountId: string,
): Promise<void> {
  return invoke<void>('activate_account', { providerId, accountId })
}

export function deleteAccount(
  providerId: string,
  accountId: string,
): Promise<void> {
  return invoke<void>('delete_account', { providerId, accountId })
}

export function listQuota(): Promise<QuotaSnapshot[]> {
  return invoke<QuotaSnapshot[]>('list_quota')
}
