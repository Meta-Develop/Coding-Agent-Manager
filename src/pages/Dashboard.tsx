import { useEffect, useState } from 'react'
import InitialMark from '@/components/InitialMark'
import PageHeader from '@/components/PageHeader'
import { listAccounts, listProviders, listQuota } from '@/lib/tauri'
import type {
  ProviderAccountList,
  ProviderDescriptor,
  ProviderQuotaList,
  QuotaSnapshot,
} from '@/types'

type QuotaView = 'list' | 'grid'

/** What provider adapters can honestly report on this machine. */
export default function Dashboard() {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([])
  const [listings, setListings] = useState<ProviderAccountList[]>([])
  const [summaryError, setSummaryError] = useState<string | null>(null)
  const [summaryLoading, setSummaryLoading] = useState(true)
  const [quota, setQuota] = useState<ProviderQuotaList[]>([])
  const [quotaError, setQuotaError] = useState<string | null>(null)
  const [quotaLoading, setQuotaLoading] = useState(true)
  const [quotaView, setQuotaView] = useState<QuotaView>('grid')

  useEffect(() => {
    let cancelled = false

    Promise.allSettled([
      Promise.resolve().then(listProviders),
      Promise.resolve().then(() => listAccounts()),
    ]).then(([providerResult, listingResult]) => {
      if (cancelled) return

      const failures: string[] = []
      if (providerResult.status === 'fulfilled') {
        setProviders(providerResult.value)
      } else {
        failures.push(`providers: ${String(providerResult.reason)}`)
      }
      if (listingResult.status === 'fulfilled') {
        setListings(listingResult.value)
      } else {
        failures.push(`accounts: ${String(listingResult.reason)}`)
      }
      setSummaryError(failures.length === 0 ? null : failures.join('; '))
      setSummaryLoading(false)
    })

    Promise.resolve()
      .then(listQuota)
      .then((nextQuota) => {
        if (cancelled) return
        setQuota(nextQuota)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setQuotaError(String(cause))
      })
      .finally(() => {
        if (cancelled) return
        setQuotaLoading(false)
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
        description="Provider and account state, plus only the quota signals each adapter can source."
      />
      {summaryLoading && (
        <p className="text-sm text-ink-muted" role="status">
          Loading summary…
        </p>
      )}
      {summaryError !== null && (
        <p className="notice notice-danger mb-4" role="alert">
          Failed to load dashboard summary: {summaryError}
        </p>
      )}
      {!summaryLoading && providers.length === 0 && (
        <p className="text-sm text-ink-muted">No providers were returned.</p>
      )}
      {!summaryLoading && providers.length > 0 && (
        <div className="table-frame">
          <table className="data-table">
            <thead>
              <tr>
                <th scope="col">What is known</th>
                <th scope="col">On this machine</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Providers detected
                </th>
                <td className="text-ink-muted">
                  {detected.length} of {providers.length}
                  {detected.length === 0
                    ? ' — none detected'
                    : ` — ${detected.map((provider) => provider.displayName).join(', ')}`}
                </td>
              </tr>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Visible accounts
                </th>
                <td className="text-ink-muted">{visibleCount}</td>
              </tr>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Adapters that cannot list accounts
                </th>
                <td className="text-ink-muted">
                  {displayNames(cannotList, providers)}
                </td>
              </tr>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Lookups that failed
                </th>
                <td className="text-ink-muted">
                  {displayNames(failed, providers)}
                </td>
              </tr>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Listings whose live login file is damaged
                </th>
                <td className="text-ink-muted">
                  {displayNames(listedWithError, providers)}
                </td>
              </tr>
              <tr>
                <th scope="row" className="text-left font-medium">
                  Listings that see only an API key
                </th>
                <td className="text-ink-muted">
                  {displayNames(apiKeyOnly, providers)}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      )}

      <section className="mt-8" aria-labelledby="quota-heading">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2
              id="quota-heading"
              className="text-lg font-semibold tracking-tight"
            >
              Provider quota
            </h2>
            <p className="mt-1 text-sm text-ink-muted">
              Remaining quota is derived only from sourced utilization.
            </p>
          </div>
          <div
            className="flex rounded-md border border-border-subtle bg-surface p-1 shadow-control"
            role="group"
            aria-label="Quota view"
          >
            <ViewButton
              selected={quotaView === 'list'}
              onClick={() => setQuotaView('list')}
            >
              List
            </ViewButton>
            <ViewButton
              selected={quotaView === 'grid'}
              onClick={() => setQuotaView('grid')}
            >
              Grid
            </ViewButton>
          </div>
        </div>

        {quotaLoading && (
          <p className="mt-4 text-sm text-ink-muted" role="status">
            Loading quota…
          </p>
        )}
        {!summaryLoading && providers.length > 0 && (
          <ul
            className={
              quotaView === 'grid'
                ? 'mt-4 grid gap-4 md:grid-cols-2'
                : 'mt-4 space-y-3'
            }
            aria-label={quotaView === 'grid' ? 'Quota grid' : 'Quota list'}
          >
            {providers.map((provider) => (
              <li key={provider.id}>
                <ProviderQuota
                  provider={provider}
                  result={quota.find(
                    (candidate) => candidate.providerId === provider.id,
                  )}
                  requestError={quotaError}
                  loading={quotaLoading}
                  view={quotaView}
                />
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  )
}

function ViewButton({
  selected,
  onClick,
  children,
}: {
  selected: boolean
  onClick: () => void
  children: string
}) {
  return (
    <button
      type="button"
      className={`rounded px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
        selected
          ? 'bg-surface-raised font-medium text-ink shadow-control ring-1 ring-accent'
          : 'text-ink-muted hover:text-ink'
      }`}
      aria-pressed={selected}
      onClick={onClick}
    >
      {children}
    </button>
  )
}

function ProviderQuota({
  provider,
  result,
  requestError,
  loading,
  view,
}: {
  provider: ProviderDescriptor
  result: ProviderQuotaList | undefined
  requestError: string | null
  loading: boolean
  view: QuotaView
}) {
  const planLabel = result?.planLabel?.trim() || 'Unavailable'

  return (
    <article
      className={`panel h-full p-4 ${
        view === 'list'
          ? 'md:grid md:grid-cols-[minmax(12rem,1fr)_2fr] md:gap-6'
          : ''
      }`}
      aria-label={`${provider.displayName} quota`}
    >
      <header className="flex items-start gap-3">
        <InitialMark name={provider.displayName} />
        <div>
          <h3 className="font-semibold tracking-tight">
            {provider.displayName}
          </h3>
          <dl className="mt-2 space-y-1 text-sm text-ink-muted">
            <div className="flex gap-1">
              <dt>Adapter maturity:</dt>
              <dd>{provider.maturity}</dd>
            </div>
            <div className="flex gap-1">
              <dt>Plan:</dt>
              <dd>{planLabel}</dd>
            </div>
          </dl>
        </div>
      </header>
      <div className={view === 'list' ? 'mt-3 md:mt-0' : 'mt-3'}>
        <QuotaOutcome
          result={result}
          requestError={requestError}
          loading={loading}
        />
      </div>
    </article>
  )
}

function QuotaOutcome({
  result,
  requestError,
  loading,
}: {
  result: ProviderQuotaList | undefined
  requestError: string | null
  loading: boolean
}) {
  if (loading) {
    return <p className="text-sm text-ink-muted">Checking quota signal…</p>
  }
  if (requestError !== null) {
    return <QuotaFailure message={`Quota collection failed: ${requestError}`} />
  }
  if (result === undefined) {
    return <QuotaFailure message="Quota result missing for this provider." />
  }
  if (result.outcome.kind === 'no-signal') {
    return <p className="text-sm text-ink-muted">No quota signal available</p>
  }
  if (result.outcome.kind === 'failed') {
    return (
      <QuotaFailure
        message={`Quota collection failed: ${result.outcome.error.message}`}
      />
    )
  }
  if (result.snapshots.length === 0) {
    return (
      <QuotaFailure message="Quota result is available but contains no sourced snapshots." />
    )
  }

  return (
    <ul className="space-y-3" aria-label="Sourced quota snapshots">
      {result.snapshots.map((snapshot, index) => (
        <li
          key={`${snapshot.accountId}-${snapshot.model ?? 'all'}-${snapshot.capturedAt}-${index}`}
          className="rounded-md border border-border-subtle bg-surface p-3 shadow-control"
        >
          <QuotaSnapshotDetails snapshot={snapshot} />
        </li>
      ))}
    </ul>
  )
}

function QuotaFailure({ message }: { message: string }) {
  return (
    <p className="notice notice-danger" role="alert">
      {message}
    </p>
  )
}

function QuotaSnapshotDetails({ snapshot }: { snapshot: QuotaSnapshot }) {
  if (
    !Number.isFinite(snapshot.utilization) ||
    snapshot.utilization < 0 ||
    snapshot.utilization > 1
  ) {
    return <QuotaFailure message="Invalid utilization in quota snapshot." />
  }

  const remaining = (1 - snapshot.utilization) * 100

  return (
    <div className="text-sm">
      <p className="text-base font-semibold">
        {formatPercentage(remaining)} remaining
      </p>
      <dl className="mt-2 space-y-1 text-ink-muted">
        <Detail label="Account" value={snapshot.accountId} />
        <Detail label="Model" value={snapshot.model ?? 'Not specified'} />
        <Detail
          label="Window"
          value={snapshot.windowLabel ?? 'Not published'}
        />
        <div className="flex flex-wrap gap-x-1">
          <dt>Reset:</dt>
          <dd>
            <Timestamp value={snapshot.resetsAt} absent="Not published" />
          </dd>
        </div>
        <div className="flex flex-wrap gap-x-1">
          <dt>Source:</dt>
          <dd>
            <code>{snapshot.source}</code>
          </dd>
        </div>
        <div className="flex flex-wrap gap-x-1">
          <dt>Captured:</dt>
          <dd>
            <CapturedAt value={snapshot.capturedAt} />
          </dd>
        </div>
      </dl>
    </div>
  )
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-wrap gap-x-1">
      <dt>{label}:</dt>
      <dd>{value}</dd>
    </div>
  )
}

function Timestamp({
  value,
  absent,
}: {
  value: string | null
  absent: string
}) {
  if (value === null) return <>{absent}</>
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) {
    return <span>Invalid timestamp ({value})</span>
  }
  return (
    <time dateTime={value} title={value}>
      {new Date(timestamp).toLocaleString()}
    </time>
  )
}

function CapturedAt({ value }: { value: string }) {
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) {
    return <span>Invalid timestamp ({value})</span>
  }
  if (timestamp > Date.now()) {
    return (
      <time dateTime={value} title={value}>
        Timestamp is in the future
      </time>
    )
  }
  return (
    <time dateTime={value} title={value}>
      {formatAge(Date.now() - timestamp)}
    </time>
  )
}

function formatPercentage(value: number): string {
  return `${new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 }).format(value)}%`
}

function formatAge(elapsedMilliseconds: number): string {
  const seconds = Math.floor(elapsedMilliseconds / 1_000)
  if (seconds < 60) return `${seconds} ${plural(seconds, 'second')} ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} ${plural(minutes, 'minute')} ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} ${plural(hours, 'hour')} ago`
  const days = Math.floor(hours / 24)
  return `${days} ${plural(days, 'day')} ago`
}

function plural(value: number, unit: string): string {
  return value === 1 ? unit : `${unit}s`
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
