import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import type { PendingKind } from '@/components/AccountActions'
import { activateAccount, addAccount, deleteAccount } from '@/lib/tauri'
import type { ProviderDescriptor } from '@/types'

export interface PendingConfirmation {
  providerId: string
  accountId: string
  kind: PendingKind
}

export interface Notice {
  tone: 'progress' | 'success' | 'failure'
  message: string
}

interface AccountMutationValue {
  busy: boolean
  notice: Notice | null
  pending: PendingConfirmation | null
  /** Bumped after a successful command so a mounted page re-fetches. */
  listingEpoch: number
  requestPending: (pending: PendingConfirmation) => void
  cancelPending: () => void
  runMutation: (
    kind: PendingKind | 'add',
    provider: ProviderDescriptor,
    accountId: string,
    accountName: string,
  ) => Promise<boolean>
}

const AccountMutationContext = createContext<AccountMutationValue | null>(null)

/**
 * Holds in-flight add/switch/delete across route changes. The Accounts
 * page unmounts when the sidebar is used; `add_account` can block for as
 * long as `codex login` takes in the launching terminal. That state has
 * to live here so a second sign-in cannot be started and so returning to
 * Accounts still shows the running operation. Unmounting is not
 * cancellation: this application cannot cancel a vendor login already
 * running in the terminal.
 */
export function AccountMutationProvider({ children }: { children: ReactNode }) {
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState<Notice | null>(null)
  const [pending, setPending] = useState<PendingConfirmation | null>(null)
  const [listingEpoch, setListingEpoch] = useState(0)
  const busyRef = useRef(false)

  const requestPending = useCallback((next: PendingConfirmation) => {
    if (busyRef.current) return
    setPending(next)
  }, [])

  const cancelPending = useCallback(() => {
    setPending(null)
  }, [])

  const runMutation = useCallback(
    async (
      kind: PendingKind | 'add',
      provider: ProviderDescriptor,
      accountId: string,
      accountName: string,
    ): Promise<boolean> => {
      if (busyRef.current) {
        return false
      }
      busyRef.current = true
      setPending(null)
      setBusy(true)
      setNotice({
        tone: 'progress',
        message: progressMessage(kind, provider, accountId, accountName),
      })
      try {
        try {
          if (kind === 'add') {
            await addAccount(provider.id, accountId)
          } else if (kind === 'switch') {
            await activateAccount(provider.id, accountId)
          } else {
            await deleteAccount(provider.id, accountId)
          }
        } catch (cause: unknown) {
          setNotice({
            tone: 'failure',
            message: `${failureLead(kind, provider, accountId, accountName)} ${commandErrorMessage(cause)}`,
          })
          return false
        }
        setListingEpoch((epoch) => epoch + 1)
        setNotice({
          tone: 'success',
          message: successMessage(kind, provider, accountId, accountName),
        })
        return true
      } finally {
        busyRef.current = false
        setBusy(false)
      }
    },
    [],
  )

  const value = useMemo<AccountMutationValue>(
    () => ({
      busy,
      notice,
      pending,
      listingEpoch,
      requestPending,
      cancelPending,
      runMutation,
    }),
    [
      busy,
      notice,
      pending,
      listingEpoch,
      requestPending,
      cancelPending,
      runMutation,
    ],
  )

  return (
    <AccountMutationContext.Provider value={value}>
      {children}
    </AccountMutationContext.Provider>
  )
}

export function useAccountMutation(): AccountMutationValue {
  const value = useContext(AccountMutationContext)
  if (value === null) {
    throw new Error(
      'useAccountMutation must be used within AccountMutationProvider',
    )
  }
  return value
}

/** Visible from every page so a running mutation is not an Accounts-only fact. */
export function MutationNotice() {
  const { notice } = useAccountMutation()
  if (notice === null) {
    return null
  }
  return (
    <p
      key={notice.message}
      role={notice.tone === 'failure' ? 'alert' : 'status'}
      className={noticeClass(notice.tone)}
    >
      {notice.message}
    </p>
  )
}

function noticeClass(tone: Notice['tone']): string {
  if (tone === 'failure') {
    return 'mb-4 rounded-md border border-border-subtle p-3 text-sm'
  }
  if (tone === 'progress') {
    return 'mb-4 rounded-md border border-border-subtle bg-surface-raised p-3 text-sm'
  }
  return 'mb-4 text-sm text-ink-muted'
}

function progressMessage(
  kind: PendingKind | 'add',
  provider: ProviderDescriptor,
  accountId: string,
  accountName: string,
): string {
  if (kind === 'add') {
    return `Adding ${accountId} to ${provider.displayName}. Sign-in is waiting in the terminal that launched this application; this window will update when it finishes. Closing this window does not cancel that sign-in, and this application cannot cancel it either.`
  }
  if (kind === 'switch') {
    return `Switching ${provider.displayName} to ${accountName}…`
  }
  return `Deleting this application's stored copy of ${accountName}…`
}

function successMessage(
  kind: PendingKind | 'add',
  provider: ProviderDescriptor,
  accountId: string,
  accountName: string,
): string {
  if (kind === 'add') {
    return `Added ${accountId} to ${provider.displayName}.`
  }
  if (kind === 'switch') {
    return `Switched ${provider.displayName} to ${accountName}.`
  }
  return `Deleted this application's stored copy of ${accountName}.`
}

function failureLead(
  kind: PendingKind | 'add',
  provider: ProviderDescriptor,
  accountId: string,
  accountName: string,
): string {
  if (kind === 'add') {
    return `Could not add ${accountId} to ${provider.displayName}:`
  }
  if (kind === 'switch') {
    return `Could not switch ${provider.displayName} to ${accountName}:`
  }
  return `Could not delete ${accountName}:`
}

function commandErrorMessage(cause: unknown): string {
  if (typeof cause === 'string' && cause.trim() !== '') {
    return cause
  }
  if (cause instanceof Error && cause.message.trim() !== '') {
    return cause.message
  }
  return 'The command failed without an error message.'
}
