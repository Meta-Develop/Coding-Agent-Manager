import { useEffect, useRef, useState, type ReactElement } from 'react'
import AccountActions, {
  accountDisplayName,
  type PendingKind,
} from '@/components/AccountActions'
import AddAccountForm from '@/components/AddAccountForm'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import {
  activateAccount,
  addAccount,
  deleteAccount,
  listAccounts,
  listProviders,
} from '@/lib/tauri'
import type {
  Account,
  ProviderAccountList,
  ProviderCapability,
  ProviderDescriptor,
} from '@/types'

/**
 * Lists accounts from every adapter, grouped by provider. Empty, unfinished,
 * failed, and API-key-only listings are distinct (`NFR-8`). Add appears when
 * the adapter advertises it. Switch and delete appear only when the adapter
 * advertises the matching capability and the row is a stored copy this
 * application can act on (`FR-1`, `NFR-8`).
 */
export default function Accounts() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [listings, setListings] = useState<ProviderAccountList[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState<Notice | null>(null)
  const [pending, setPending] = useState<PendingConfirmation | null>(null)
  const cancelledRef = useRef(false)

  useEffect(() => {
    cancelledRef.current = false
    let cancelled = false
    Promise.all([listProviders(), listAccounts()])
      .then(([nextProviders, nextListings]) => {
        if (cancelled) return
        setProviders(nextProviders)
        setListings(nextListings)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setError(String(cause))
      })
      .finally(() => {
        if (cancelled) return
        setLoading(false)
      })
    return () => {
      cancelled = true
      cancelledRef.current = true
    }
  }, [])

  const cannotSwitch = providers.filter(
    (provider) => !hasCapability(provider, 'switch-account'),
  )

  async function runMutation(
    kind: PendingKind | 'add',
    provider: ProviderDescriptor,
    accountId: string,
    accountName: string,
  ): Promise<boolean> {
    setPending(null)
    setBusy(true)
    setNotice({
      tone: 'progress',
      message: progressMessage(kind, provider, accountId, accountName),
    })

    try {
      if (kind === 'add') {
        await addAccount(provider.id, accountId)
      } else if (kind === 'switch') {
        await activateAccount(provider.id, accountId)
      } else {
        await deleteAccount(provider.id, accountId)
      }
    } catch (cause: unknown) {
      if (cancelledRef.current) return false
      setNotice({
        tone: 'failure',
        message: `${failureLead(kind, provider, accountId, accountName)} ${commandErrorMessage(cause)}`,
      })
      setBusy(false)
      return false
    }

    if (cancelledRef.current) return true

    try {
      const [nextProviders, nextListings] = await Promise.all([
        listProviders(),
        listAccounts(),
      ])
      if (cancelledRef.current) return true
      setProviders(nextProviders)
      setListings(nextListings)
      setNotice({
        tone: 'success',
        message: successMessage(kind, provider, accountId, accountName),
      })
    } catch (cause: unknown) {
      if (cancelledRef.current) return true
      setNotice({
        tone: 'failure',
        message: `${successMessage(kind, provider, accountId, accountName)} Reloading the list failed: ${commandErrorMessage(cause)}`,
      })
    } finally {
      if (!cancelledRef.current) {
        setBusy(false)
      }
    }

    return true
  }

  function handleConfirmPending() {
    if (pending === null) return
    const provider = providers.find((item) => item.id === pending.providerId)
    if (provider === undefined) {
      setPending(null)
      return
    }
    const listing = listingFor(listings, pending.providerId)
    const account = listing?.accounts.find(
      (item) => item.id === pending.accountId,
    )
    const accountName =
      account === undefined ? pending.accountId : accountDisplayName(account)
    void runMutation(pending.kind, provider, pending.accountId, accountName)
  }

  return (
    <>
      <PageHeader
        title="Accounts"
        description="Every account this application can see, grouped by provider. Add, switch, and delete appear only where an adapter implements them."
      />
      {loading && (
        <p className="text-sm text-ink-muted" role="status">
          Loading accounts…
        </p>
      )}
      {error !== null && (
        <p className="mb-4 rounded-md border border-border-subtle p-3 text-sm">
          Failed to load accounts: {error}
        </p>
      )}
      {notice !== null && (
        <p
          key={notice.message}
          role={notice.tone === 'failure' ? 'alert' : 'status'}
          className={noticeClass(notice.tone)}
        >
          {notice.message}
        </p>
      )}
      {!loading && error === null && (
        <div className="space-y-8" aria-busy={busy}>
          {providers.length === 0 && (
            <p className="text-sm text-ink-muted">
              No providers were returned.
            </p>
          )}
          {providers.map((provider) => (
            <ProviderAccounts
              key={provider.id}
              provider={provider}
              listing={listingFor(listings, provider.id)}
              busy={busy}
              pending={
                pending !== null && pending.providerId === provider.id
                  ? pending
                  : null
              }
              onAdd={(accountId) =>
                runMutation('add', provider, accountId, accountId)
              }
              onRequest={(accountId, kind) =>
                setPending({
                  providerId: provider.id,
                  accountId,
                  kind,
                })
              }
              onCancelPending={() => setPending(null)}
              onConfirmPending={handleConfirmPending}
            />
          ))}
          {cannotSwitch.length > 0 && (
            <NotImplemented requirement="FR-1">
              {cannotSwitch.map((provider) => provider.displayName).join(', ')}{' '}
              cannot switch accounts yet. This page lists what those adapters
              can see; it does not change which account those tools will use.
            </NotImplemented>
          )}
        </div>
      )}
    </>
  )
}

interface PendingConfirmation {
  providerId: string
  accountId: string
  kind: PendingKind
}

interface Notice {
  tone: 'progress' | 'success' | 'failure'
  message: string
}

function ProviderAccounts({
  provider,
  listing,
  busy,
  pending,
  onAdd,
  onRequest,
  onCancelPending,
  onConfirmPending,
}: {
  provider: ProviderDescriptor
  listing: ProviderAccountList | undefined
  busy: boolean
  pending: PendingConfirmation | null
  onAdd: (accountId: string) => Promise<boolean>
  onRequest: (accountId: string, kind: PendingKind) => void
  onCancelPending: () => void
  onConfirmPending: () => void
}) {
  const headingId = `accounts-heading-${provider.id}`
  const canAdd = hasCapability(provider, 'add-account')
  const canSwitch = hasCapability(provider, 'switch-account')
  const canDelete = hasCapability(provider, 'delete-account')

  return (
    <section aria-labelledby={headingId}>
      <h2 id={headingId} className="text-base font-semibold">
        {provider.displayName}
      </h2>
      <p className="mt-1 text-xs text-ink-muted">
        {provider.vendor} · {provider.maturity}
      </p>
      {listingBody({
        headingId,
        listing,
        provider,
        busy,
        pending,
        canSwitch,
        canDelete,
        onRequest,
        onCancelPending,
        onConfirmPending,
      })}
      {canAdd && listingHasUnstored(listing) && (
        <p className="mt-3 text-sm text-ink-muted">
          The tool&apos;s current identity is not stored here, so this
          application cannot switch back to it.
        </p>
      )}
      {canAdd && (
        <AddAccountForm provider={provider} disabled={busy} onAdd={onAdd} />
      )}
    </section>
  )
}

function listingBody({
  headingId,
  listing,
  provider,
  busy,
  pending,
  canSwitch,
  canDelete,
  onRequest,
  onCancelPending,
  onConfirmPending,
}: {
  headingId: string
  listing: ProviderAccountList | undefined
  provider: ProviderDescriptor
  busy: boolean
  pending: PendingConfirmation | null
  canSwitch: boolean
  canDelete: boolean
  onRequest: (accountId: string, kind: PendingKind) => void
  onCancelPending: () => void
  onConfirmPending: () => void
}): ReactElement {
  if (listing === undefined) {
    return (
      <p className="mt-3 text-sm text-ink-muted">
        No listing was returned for this provider.
      </p>
    )
  }

  const table = (accounts: Account[]) => (
    <AccountTable
      headingId={headingId}
      accounts={accounts}
      provider={provider}
      busy={busy}
      pending={pending}
      canSwitch={canSwitch}
      canDelete={canDelete}
      onRequest={onRequest}
      onCancelPending={onCancelPending}
      onConfirmPending={onConfirmPending}
    />
  )

  switch (listing.outcome.kind) {
    case 'listed':
      if (listing.accounts.length === 0) {
        return (
          <p className="mt-3 text-sm text-ink-muted">
            Nothing is configured for this provider on this machine.
          </p>
        )
      }
      return table(listing.accounts)
    case 'listed-api-key-only':
      if (listing.accounts.length === 0) {
        return (
          <LimitationNote>
            No GEMINI_API_KEY is set. This adapter cannot see Google OAuth
            accounts.
          </LimitationNote>
        )
      }
      return (
        <>
          {table(listing.accounts)}
          <LimitationNote>
            This adapter only sees GEMINI_API_KEY. It cannot see Google OAuth
            accounts.
          </LimitationNote>
        </>
      )
    case 'not-implemented':
      return (
        <LimitationNote>This adapter cannot list accounts yet.</LimitationNote>
      )
    case 'failed':
      return (
        <p className="mt-3 rounded-md border border-border-subtle p-3 text-sm">
          Looking failed: {listing.outcome.error.message}
        </p>
      )
    default: {
      const _exhaustive: never = listing.outcome
      return _exhaustive
    }
  }
}

function LimitationNote({ children }: { children: React.ReactNode }) {
  return (
    <div className="mt-3 rounded-lg border border-dashed border-border-subtle p-4">
      <p className="text-sm text-ink-muted">{children}</p>
    </div>
  )
}

function AccountTable({
  headingId,
  accounts,
  provider,
  busy,
  pending,
  canSwitch,
  canDelete,
  onRequest,
  onCancelPending,
  onConfirmPending,
}: {
  headingId: string
  accounts: Account[]
  provider: ProviderDescriptor
  busy: boolean
  pending: PendingConfirmation | null
  canSwitch: boolean
  canDelete: boolean
  onRequest: (accountId: string, kind: PendingKind) => void
  onCancelPending: () => void
  onConfirmPending: () => void
}) {
  const unknownActiveId = `${headingId}-unknown-active`
  const activeKnown = accounts.some((account) => account.isActive)
  const showActions =
    (canSwitch || canDelete) && accounts.some((account) => account.isStored)

  return (
    <>
      {!activeKnown && (
        <p id={unknownActiveId} className="mt-3 text-sm text-ink-muted">
          Active account is not known for this provider.
        </p>
      )}
      <div className="mt-3 overflow-x-auto">
        <table
          className="w-full text-left text-sm"
          aria-labelledby={headingId}
          aria-describedby={activeKnown ? undefined : unknownActiveId}
        >
          <thead className="text-xs uppercase tracking-wide text-ink-muted">
            <tr>
              <th scope="col" className="py-2 pr-4">
                Label
              </th>
              <th scope="col" className="py-2 pr-4">
                Identity
              </th>
              <th scope="col" className="py-2 pr-4">
                Auth
              </th>
              <th scope="col" className="py-2 pr-4">
                Status
              </th>
              <th scope="col" className={showActions ? 'py-2 pr-4' : 'py-2'}>
                Expires
              </th>
              {showActions && (
                <th scope="col" className="py-2">
                  Actions
                </th>
              )}
            </tr>
          </thead>
          <tbody>
            {accounts.map((account) => (
              <tr key={account.id} className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  {presentOrAbsent(account.label, 'No label')}
                </th>
                <td className="py-2 pr-4 text-ink-muted">
                  {presentOrAbsent(account.maskedIdentity, 'Not established')}
                </td>
                <td className="py-2 pr-4 text-ink-muted">{account.authKind}</td>
                <td className="py-2 pr-4 text-ink-muted">
                  {statusLabel(account, activeKnown)}
                </td>
                <td
                  className={
                    showActions
                      ? 'py-2 pr-4 text-ink-muted'
                      : 'py-2 text-ink-muted'
                  }
                >
                  {presentOrAbsent(account.expiresAt, 'Not established')}
                </td>
                {showActions && (
                  <td className="py-2">
                    <AccountActions
                      account={account}
                      provider={provider}
                      canSwitch={canSwitch && account.isStored}
                      canDelete={canDelete && account.isStored}
                      disabled={busy}
                      pending={
                        pending !== null && pending.accountId === account.id
                          ? pending.kind
                          : null
                      }
                      onRequest={(kind) => onRequest(account.id, kind)}
                      onCancel={onCancelPending}
                      onConfirm={onConfirmPending}
                    />
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}

function listingFor(
  listings: ProviderAccountList[],
  providerId: string,
): ProviderAccountList | undefined {
  return listings.find((listing) => listing.providerId === providerId)
}

function listingHasUnstored(listing: ProviderAccountList | undefined): boolean {
  return (
    listing !== undefined &&
    listing.accounts.some((account) => !account.isStored)
  )
}

function presentOrAbsent(
  value: string | null,
  absent: string,
): string | ReactElement {
  if (value === null || value.trim() === '') {
    return <span className="text-ink-muted">{absent}</span>
  }
  return value
}

/**
 * `isActive: false` on every row is not a negative check — the adapter
 * did not establish which identity the tool will use. Only say "Active"
 * when at least one row in the group is marked active.
 */
function statusLabel(account: Account, activeKnown: boolean): string {
  if (!activeKnown) {
    return 'Not known'
  }
  return account.isActive ? 'Active' : '—'
}

function hasCapability(
  provider: ProviderDescriptor,
  capability: ProviderCapability,
): boolean {
  return provider.capabilities.includes(capability)
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
    return `Adding ${accountId} to ${provider.displayName}. Sign-in is waiting in the terminal that launched this application; this window will update when it finishes.`
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
