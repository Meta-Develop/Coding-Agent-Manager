import PageHeader from '@/components/PageHeader'
import NotImplemented from '@/components/NotImplemented'

export default function Relay() {
  return (
    <>
      <PageHeader
        title="Relay"
        description="Local HTTP endpoint that adapts between OpenAI, Anthropic, and Gemini wire formats."
      />
      <NotImplemented requirement="FR-6">
        The relay binds to loopback only by default. Exposing it on another
        interface is an explicit, warned opt-in — see
        <code className="mx-1">docs/SECURITY_MODEL.md</code>.
      </NotImplemented>
    </>
  )
}
