import { useEffect, useState } from 'react'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import { listAccounts, listProviders } from '@/lib/tauri'
import type { ProviderAccountList, ProviderDescriptor } from '@/types'

/**
 * What is known on this machine. Quota (`FR-5`) stays a placeholder because
 * no adapter publishes a usage signal (`NFR-8`). Account counts and listing
 * capability come from per-provider outcomes, not from adapter maturity.
 */
export default function Dashboard() {
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

  const detected = providers.filter(
    (provider) => provider.installState === 'installed',
  )
  const visibleCount = listings.reduce(
    (count, listing) => count + listing.accounts.length,
    0,
  )
  const cannotList = listings.filter(
    (listing) => listing.outcome.kind === 'not-implemented',
  )
  const failed = listings.filter((listing) => listing.outcome.kind === 'failed')
  const listedWithError = listings.filter(
    (listing) => listing.outcome.kind === 'listed-with-error',
  )
  const apiKeyOnly = listings.filter(
    (listing) => listing.outcome.kind === 'listed-api-key-only',
  )

  return (
    <>
      <PageHeader
        title="Dashboard"
        description="What is known on this machine. Quota is not shown because no adapter publishes a usage signal."
      />
      {loading && (
        <p className="text-sm text-ink-muted" role="status">
          Loading summary…
        </p>
      )}
      {error !== null && (
        <p className="mb-4 rounded-md border border-border-subtle p-3 text-sm">
          Failed to load dashboard: {error}
        </p>
      )}
      {!loading && error === null && providers.length === 0 && (
        <p className="text-sm text-ink-muted">No providers were returned.</p>
      )}
      {!loading && error === null && providers.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase tracking-wide text-ink-muted">
              <tr>
                <th scope="col" className="py-2 pr-4">
                  What is known
                </th>
                <th scope="col" className="py-2">
                  On this machine
                </th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Providers detected
                </th>
                <td className="py-2 text-ink-muted">
                  {detected.length} of {providers.length}
                  {detected.length === 0
                    ? ' — none detected'
                    : ` — ${detected.map((provider) => provider.displayName).join(', ')}`}
                </td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Visible accounts
                </th>
                <td className="py-2 text-ink-muted">{visibleCount}</td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Adapters that cannot list accounts
                </th>
                <td className="py-2 text-ink-muted">
                  {displayNames(cannotList, providers)}
                </td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Lookups that failed
                </th>
                <td className="py-2 text-ink-muted">
                  {displayNames(failed, providers)}
                </td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Listings whose live login file is damaged
                </th>
                <td className="py-2 text-ink-muted">
                  {displayNames(listedWithError, providers)}
                </td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Listings that see only an API key
                </th>
                <td className="py-2 text-ink-muted">
                  {displayNames(apiKeyOnly, providers)}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
      <div className="mt-8">
        <NotImplemented requirement="FR-5">
          The quota dashboard depends on per-provider usage signals. See
          <code className="mx-1">docs/PROVIDER_MATRIX.md</code>
          for which providers expose one.
        </NotImplemented>
      </div>
    </>
  )
}

function displayNames(
  listings: ProviderAccountList[],
  providers: ProviderDescriptor[],
): string {
  if (listings.length === 0) {
    return 'None'
  }
  return listings
    .map(
      (listing) =>
        providers.find((provider) => provider.id === listing.providerId)
          ?.displayName ?? listing.providerId,
    )
    .join(', ')
}
