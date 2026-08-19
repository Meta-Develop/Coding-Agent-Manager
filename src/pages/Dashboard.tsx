import { useEffect, useState } from 'react'
import NotImplemented from '@/components/NotImplemented'
import PageHeader from '@/components/PageHeader'
import { listAccounts, listProviders } from '@/lib/tauri'
import type { Account, ProviderDescriptor } from '@/types'

/**
 * What is known on this machine. Quota (`FR-5`) stays a placeholder because
 * no adapter publishes a usage signal (`NFR-8`).
 */
export default function Dashboard() {
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

  const detected = providers.filter(
    (provider) => provider.installState === 'installed',
  )
  const cannotList = providers.filter(
    (provider) => provider.maturity === 'planned',
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
                <td className="py-2 text-ink-muted">{accounts.length}</td>
              </tr>
              <tr className="border-t border-border-subtle">
                <th scope="row" className="py-2 pr-4 text-left font-medium">
                  Adapters that cannot list accounts
                </th>
                <td className="py-2 text-ink-muted">
                  {cannotList.length === 0
                    ? 'None — every adapter can list accounts'
                    : cannotList
                        .map((provider) => provider.displayName)
                        .join(', ')}
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
