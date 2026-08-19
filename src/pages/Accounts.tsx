import {
  Fragment,
  useEffect,
  useState,
  type ReactElement,
  type ReactNode,
} from 'react'
import AccountActions, {
  Confirmation,
  accountDisplayName,
  type PendingKind,
} from '@/components/AccountActions'
import AddAccountForm from '@/components/AddAccountForm'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import {
  useAccountMutation,
  type PendingConfirmation,
} from '@/lib/accountMutation'
import { listAccounts, listProviders } from '@/lib/tauri'
import type {
  Account,
  ProviderAccountList,
  ProviderCapability,
  ProviderDescriptor,
} from '@/types'

/**
 * Lists accounts from every adapter, grouped by provider. Empty, unfinished,
 * failed, listed-with-error, and API-key-only listings are distinct (`NFR-8`).
 * Add appears when the adapter advertises it. Switch appears only when the
 * adapter advertises it, the row is a stored copy, and that copy is
 * complete. Delete appears when the adapter advertises it and the row is
 * stored, including incomplete slots (`FR-1`, `NFR-8`).
 */
export default function Accounts() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [listings, setListings] = useState<ProviderAccountList[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const {
    busy,
    pending,
    listingEpoch,
    requestPending,
    cancelPending,
    runMutation,
  } = useAccountMutation()

  useEffect(() => {
    let cancelled = false
    Promise.all([listProviders(), listAccounts()])
      .then(([nextProviders, nextListings]) => {
        if (cancelled) return
        setProviders(nextProviders)
        setListings(nextListings)
        setError(null)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        // A reload after a successful mutation must not blank a listing
        // the user can still act on. The page-level error is only for
        // the first look.
        if (listingEpoch === 0) {
          setError(String(cause))
        }
      })
      .finally(() => {
        if (cancelled) return
        setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [listingEpoch])

  const cannotSwitch = providers.filter(
    (provider) => !hasCapability(provider, 'switch-account'),
  )

  function handleConfirmPending() {
    if (pending === null) return
    const provider = providers.find((item) => item.id === pending.providerId)
    if (provider === undefined) {
      cancelPending()
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
                requestPending({
                  providerId: provider.id,
                  accountId,
                  kind,
                })
              }
              onCancelPending={cancelPending}
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
    case 'listed-with-error':
      return (
        <>
          <DamagedLiveFileNote
            provider={provider}
            message={listing.outcome.error.message}
            path={listing.outcome.error.path}
          />
          {listing.accounts.length > 0 ? (
            table(listing.accounts)
          ) : (
            <p className="mt-3 text-sm text-ink-muted">
              No stored copy is available to switch over the damaged file.
            </p>
          )}
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

/**
 * A successful look that also found a damaged live login file. The
 * stored rows are the repair path (SPEC §7): switch one of them over
 * the file. This is the opposite of `failed`, which hides the table.
 */
function DamagedLiveFileNote({
  provider,
  message,
  path,
}: {
  provider: ProviderDescriptor
  message: string
  path: string | null
}) {
  return (
    <div className="mt-3 rounded-md border border-border-subtle p-3 text-sm">
      <p>
        {provider.displayName}&apos;s own login file is damaged, so this
        application cannot tell which account the tool would use. Switching a
        stored copy over that file (behind a restorable backup) is the recovery
        path. This application will not rewrite a file it does not understand.
      </p>
      <p className="mt-2 text-ink-muted">{textWithPath(message, path)}</p>
    </div>
  )
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
  const showExpires = accounts.some(hasExpiry)
  const showActions = accounts.some(
    (account) =>
      (canSwitch && account.isStored && !account.isIncomplete) ||
      (canDelete && account.isStored),
  )
  const columnCount = 4 + (showExpires ? 1 : 0) + (showActions ? 1 : 0)

  return (
    <>
      {!activeKnown && (
        <p id={unknownActiveId} className="mt-3 text-sm text-ink-muted">
          Active account is not known for this provider.
        </p>
      )}
      <div className="mt-3 w-0 min-w-full overflow-x-auto">
        <table
          className={
            showExpires
              ? 'accounts-table has-expires text-left text-sm'
              : 'accounts-table text-left text-sm'
          }
          aria-labelledby={headingId}
          aria-describedby={activeKnown ? undefined : unknownActiveId}
        >
          <colgroup>
            <col className="col-label" />
            <col className="col-identity" />
            <col className="col-auth" />
            <col className="col-status" />
            {showExpires && <col className="col-expires" />}
            {showActions && <col className="col-actions" />}
          </colgroup>
          <thead className="text-xs uppercase tracking-wide text-ink-muted">
            <tr>
              <th scope="col" className="py-2 pr-4 pl-3">
                Label
              </th>
              <th scope="col" className="py-2 pr-4">
                Identity
              </th>
              <th scope="col" className="py-2 pr-4">
                Auth
              </th>
              <th
                scope="col"
                className={showExpires || showActions ? 'py-2 pr-4' : 'py-2'}
              >
                Status
              </th>
              {showExpires && (
                <th scope="col" className={showActions ? 'py-2 pr-4' : 'py-2'}>
                  Expires
                </th>
              )}
              {showActions && (
                <th scope="col" className="py-2">
                  Actions
                </th>
              )}
            </tr>
          </thead>
          <tbody>
            {accounts.map((account) => {
              const labelId = `${headingId}-label-${account.id}`
              const rowId = `${headingId}-row-${account.id}`
              const confirmId = `${headingId}-confirm-${account.id}`
              const confirmRowId = `${headingId}-confirm-row-${account.id}`
              const isPending =
                pending !== null && pending.accountId === account.id
              const pendingKind = isPending ? pending.kind : null

              return (
                <Fragment key={account.id}>
                  <tr
                    id={rowId}
                    className={accountRowClass(account.isActive)}
                    aria-owns={isPending ? confirmRowId : undefined}
                    aria-describedby={isPending ? confirmId : undefined}
                  >
                    <th
                      id={labelId}
                      scope="row"
                      className="py-2 pr-4 pl-3 text-left font-medium"
                      title={
                        account.label.trim() === '' ? undefined : account.label
                      }
                    >
                      {presentOrAbsent(account.label, 'No label')}
                    </th>
                    <td className="py-2 pr-4 text-ink-muted">
                      {account.isIncomplete
                        ? 'No usable credential'
                        : presentOrAbsent(
                            account.maskedIdentity,
                            'Not established',
                          )}
                    </td>
                    <td className="py-2 pr-4 text-ink-muted">
                      {account.authKind}
                    </td>
                    <td
                      className={
                        showExpires || showActions ? 'py-2 pr-4' : 'py-2'
                      }
                    >
                      {statusCell(account, activeKnown)}
                    </td>
                    {showExpires && (
                      <td
                        className={
                          showActions
                            ? 'py-2 pr-4 text-ink-muted'
                            : 'py-2 text-ink-muted'
                        }
                      >
                        {formatExpiry(account.expiresAt)}
                      </td>
                    )}
                    {showActions && (
                      <td className="py-2">
                        {pendingKind === null && (
                          <AccountActions
                            account={account}
                            canSwitch={
                              canSwitch &&
                              account.isStored &&
                              !account.isIncomplete
                            }
                            canDelete={canDelete && account.isStored}
                            disabled={busy}
                            onRequest={(kind) => onRequest(account.id, kind)}
                          />
                        )}
                      </td>
                    )}
                  </tr>
                  {pendingKind !== null && pending !== null && (
                    <tr
                      id={confirmRowId}
                      className="border-t border-border-subtle bg-surface-raised"
                    >
                      <td
                        colSpan={columnCount}
                        headers={labelId}
                        className="py-3 pr-4 pl-3"
                      >
                        <Confirmation
                          id={confirmId}
                          label={
                            pendingKind === 'switch'
                              ? `Confirm switch to ${accountDisplayName(account)}`
                              : `Confirm deletion of ${accountDisplayName(account)}`
                          }
                          confirmLabel={
                            pendingKind === 'switch'
                              ? `Confirm switch to ${accountDisplayName(account)}`
                              : `Confirm deletion of ${accountDisplayName(account)}`
                          }
                          cancelLabel={
                            pendingKind === 'switch'
                              ? 'Cancel switch'
                              : 'Cancel deletion'
                          }
                          disabled={busy}
                          onCancel={onCancelPending}
                          onConfirm={onConfirmPending}
                        >
                          {pendingKind === 'switch' ? (
                            <>
                              Switch {provider.displayName} to{' '}
                              {accountDisplayName(account)}? This replaces the
                              credential file in the tool&apos;s own home,
                              behind a restorable backup. {provider.displayName}{' '}
                              must not be running.
                            </>
                          ) : (
                            <>
                              Forget this application&apos;s stored copy of{' '}
                              {accountDisplayName(account)}?{' '}
                              {provider.displayName} is not signed out, and its
                              own files are left untouched.
                            </>
                          )}
                        </Confirmation>
                      </td>
                    </tr>
                  )}
                </Fragment>
              )
            })}
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

function hasExpiry(account: Account): boolean {
  return account.expiresAt !== null && account.expiresAt.trim() !== ''
}

/**
 * Render an expiry for a person, not a log. Unparseable values are shown
 * as the adapter produced them — they may not be timestamps at all.
 */
function formatExpiry(value: string | null): ReactNode {
  if (value === null || value.trim() === '') {
    return <span className="text-ink-muted">Not established</span>
  }
  const parsed = Date.parse(value)
  if (Number.isNaN(parsed)) {
    return value
  }
  const readable = new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(parsed))
  return (
    <time dateTime={value} title={value}>
      {readable}
    </time>
  )
}

function accountRowClass(isActive: boolean): string {
  if (isActive) {
    return 'is-active border-t border-border-subtle'
  }
  return 'border-t border-border-subtle'
}

/**
 * Incomplete is a structural fact about the stored directory, not a
 * missing active-account probe. Say that first so a blank identity is
 * not the only clue. `isActive: false` on every complete row is still
 * not a negative check — only say "Active" when at least one row is
 * marked active. The word "Active" is the state, not the colour.
 */
function statusCell(account: Account, activeKnown: boolean): ReactNode {
  if (account.isIncomplete) {
    return (
      <span className="text-ink-muted">
        Incomplete — sign-in never finished
      </span>
    )
  }
  if (!activeKnown) {
    return <span className="text-ink-muted">Not known</span>
  }
  if (account.isActive) {
    return (
      <span className="inline-flex rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-xs font-semibold text-ink">
        Active
      </span>
    )
  }
  return <span className="text-ink-muted">—</span>
}

/** Embed a filesystem path as a path, not as wrapping prose. */
function textWithPath(message: string, path: string | null): ReactNode {
  if (path === null || path === '' || !message.includes(path)) {
    return message
  }
  const pieces = message.split(path)
  return pieces.map((piece, index) => (
    <Fragment key={index}>
      {index > 0 && <code className="fs-path">{path}</code>}
      {piece}
    </Fragment>
  ))
}

function hasCapability(
  provider: ProviderDescriptor,
  capability: ProviderCapability,
): boolean {
  return provider.capabilities.includes(capability)
}
