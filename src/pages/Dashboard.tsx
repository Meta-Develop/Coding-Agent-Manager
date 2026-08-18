import PageHeader from '@/components/PageHeader'
import NotImplemented from '@/components/NotImplemented'

export default function Dashboard() {
  return (
    <>
      <PageHeader
        title="Dashboard"
        description="Active account per provider, remaining quota, and anything that needs attention."
      />
      <NotImplemented requirement="FR-5">
        The quota dashboard depends on per-provider usage signals. See
        <code className="mx-1">docs/PROVIDER_MATRIX.md</code>
        for which providers expose one.
      </NotImplemented>
    </>
  )
}
