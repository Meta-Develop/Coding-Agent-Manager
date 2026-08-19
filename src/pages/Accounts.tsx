import { useEffect, useState, type ReactElement } from 'react'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import { listAccounts, listProviders } from '@/lib/tauri'
import type { Account, ProviderAccountList, ProviderDescriptor } from '@/types'

/**
 * Lists accounts from every adapter, grouped by provider. Empty, unfinished,
 * failed, and API-key-only listings are distinct (`NFR-8`). Switching is not
 * offered (`FR-1`).
 */
export default function Accounts() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [listings, setListings] = useState<ProviderAccountList[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
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
    }
  }, [])

  return (
    <>
      <PageHeader
        title="Accounts"
        description="Every account this application can see, grouped by provider. Listing is read-only."
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
        <div className="space-y-8">
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
            />
          ))}
          <NotImplemented requirement="FR-1">
            Switching the active account is not available yet. This page lists
            what adapters can see; it does not change which account a tool will
            use.
          </NotImplemented>
        </div>
      )}
    </>
  )
}

function ProviderAccounts({
  provider,
  listing,
}: {
  provider: ProviderDescriptor
  listing: ProviderAccountList | undefined
}) {
  const headingId = `accounts-heading-${provider.id}`

  return (
    <section aria-labelledby={headingId}>
      <h2 id={headingId} className="text-base font-semibold">
        {provider.displayName}
      </h2>
      <p className="mt-1 text-xs text-ink-muted">
        {provider.vendor} · {provider.maturity}
      </p>
      {listingBody(headingId, listing)}
    </section>
  )
}

function listingBody(
  headingId: string,
  listing: ProviderAccountList | undefined,
): ReactElement {
  if (listing === undefined) {
    return (
      <p className="mt-3 text-sm text-ink-muted">
        No listing was returned for this provider.
      </p>
    )
  }

  switch (listing.outcome.kind) {
    case 'listed':
      if (listing.accounts.length === 0) {
        return (
          <p className="mt-3 text-sm text-ink-muted">
            Nothing is configured for this provider on this machine.
          </p>
        )
      }
      return <AccountTable headingId={headingId} accounts={listing.accounts} />
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
          <AccountTable headingId={headingId} accounts={listing.accounts} />
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
}: {
  headingId: string
  accounts: Account[]
}) {
  const unknownActiveId = `${headingId}-unknown-active`
  const activeKnown = accounts.some((account) => account.isActive)

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
              <th scope="col" className="py-2">
                Expires
              </th>
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
                <td className="py-2 text-ink-muted">
                  {presentOrAbsent(account.expiresAt, 'Not established')}
                </td>
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
