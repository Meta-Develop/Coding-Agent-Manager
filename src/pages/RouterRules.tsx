import PageHeader from '@/components/PageHeader'
import NotImplemented from '@/components/NotImplemented'

export default function RouterRules() {
  return (
    <>
      <PageHeader
        title="Router"
        description="Model mapping and tiered routing across accounts, driven by account type and remaining quota."
      />
      <NotImplemented requirement="FR-7">
        Routing builds on the relay and on quota signals; both must land first.
      </NotImplemented>
    </>
  )
}
