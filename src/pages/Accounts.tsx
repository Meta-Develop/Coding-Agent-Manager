import PageHeader from '@/components/PageHeader'
import NotImplemented from '@/components/NotImplemented'

export default function Accounts() {
  return (
    <>
      <PageHeader
        title="Accounts"
        description="Every account this application manages, grouped by provider. Switching is one click and never re-authenticates."
      />
      <NotImplemented requirement="FR-1">
        Account import, labelling, and switching land with the first provider
        adapters. See <code className="mx-1">docs/ROADMAP.md</code> milestone
        M2.
      </NotImplemented>
    </>
  )
}
