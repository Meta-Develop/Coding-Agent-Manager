import { useEffect, useState, type ReactElement } from 'react'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import { listAccounts, listProviders } from '@/lib/tauri'
import type { Account, ProviderDescriptor } from '@/types'

/**
 * Lists accounts from every adapter that can enumerate them, grouped by
 * provider. Adapters that cannot list yet are shown distinctly from
 * adapters that listed zero. Switching is not offered (`FR-1`).
 */
export default function Accounts() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    Promise.all([listProviders(), listAccounts()])
      .then(([nextProviders, nextAccounts]) => {
        if (cancelled) return
        setProviders(nextProviders)
        setAccounts(nextAccounts)
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

  const grouped = groupByProvider(accounts)

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
              accounts={grouped.get(provider.id) ?? []}
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
  accounts,
}: {
  provider: ProviderDescriptor
  accounts: Account[]
}) {
  const headingId = `accounts-heading-${provider.id}`
  const unknownActiveId = `accounts-unknown-active-${provider.id}`
  const activeKnown = accounts.some((account) => account.isActive)

  return (
    <section aria-labelledby={headingId}>
      <h2 id={headingId} className="text-base font-semibold">
        {provider.displayName}
      </h2>
      <p className="mt-1 text-xs text-ink-muted">
        {provider.vendor} · {provider.maturity}
      </p>
      {accounts.length > 0 ? (
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
                  <tr
                    key={account.id}
                    className="border-t border-border-subtle"
                  >
                    <th scope="row" className="py-2 pr-4 text-left font-medium">
                      {presentOrAbsent(account.label, 'No label')}
                    </th>
                    <td className="py-2 pr-4 text-ink-muted">
                      {presentOrAbsent(
                        account.maskedIdentity,
                        'Not established',
                      )}
                    </td>
                    <td className="py-2 pr-4 text-ink-muted">
                      {account.authKind}
                    </td>
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
      ) : canListAccounts(provider) ? (
        <p className="mt-3 text-sm text-ink-muted">
          No accounts found on this machine.
        </p>
      ) : (
        <div className="mt-3 rounded-lg border border-dashed border-border-subtle p-4">
          <p className="text-sm text-ink-muted">
            This adapter cannot list accounts yet.
          </p>
        </div>
      )}
    </section>
  )
}

function groupByProvider(accounts: Account[]): Map<string, Account[]> {
  const grouped = new Map<string, Account[]>()
  for (const account of accounts) {
    const existing = grouped.get(account.providerId)
    if (existing !== undefined) {
      existing.push(account)
    } else {
      grouped.set(account.providerId, [account])
    }
  }
  return grouped
}

/**
 * `planned` adapters return NotImplemented from list_accounts; the IPC
 * command skips them, so an empty group is not a successful empty listing.
 * `experimental` and `supported` enumerate, so zero rows means none found.
 */
function canListAccounts(provider: ProviderDescriptor): boolean {
  return provider.maturity !== 'planned'
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
