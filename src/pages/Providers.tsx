import { useEffect, useState } from 'react'
import PageHeader from '@/components/PageHeader'
import { listProviders } from '@/lib/tauri'
import type { ProviderDescriptor } from '@/types'

/**
 * The first end-to-end path: React -> Tauri command -> Rust provider registry.
 * If this page renders rows, the IPC surface is wired correctly.
 */
export default function Providers() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    listProviders()
      .then(setProviders)
      .catch((cause: unknown) => setError(String(cause)))
  }, [])

  return (
    <>
      <PageHeader
        title="Providers"
        description="Agent tools this application knows how to manage, and whether each was detected on this machine."
      />
      {error !== null && (
        <p className="mb-4 rounded-md border border-border-subtle p-3 text-sm">
          Failed to load providers: {error}
        </p>
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead className="text-xs uppercase tracking-wide text-ink-muted">
            <tr>
              <th className="py-2 pr-4">Provider</th>
              <th className="py-2 pr-4">Vendor</th>
              <th className="py-2 pr-4">Auth</th>
              <th className="py-2 pr-4">Adapter</th>
              <th className="py-2">Detected</th>
            </tr>
          </thead>
          <tbody>
            {providers.map((provider) => (
              <tr key={provider.id} className="border-t border-border-subtle">
                <td className="py-2 pr-4 font-medium">
                  {provider.displayName}
                </td>
                <td className="py-2 pr-4 text-ink-muted">{provider.vendor}</td>
                <td className="py-2 pr-4 text-ink-muted">
                  {provider.authKinds.join(', ')}
                </td>
                <td className="py-2 pr-4 text-ink-muted">
                  {provider.maturity}
                </td>
                <td className="py-2 text-ink-muted">{provider.installState}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}
