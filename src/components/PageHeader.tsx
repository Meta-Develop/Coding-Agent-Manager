interface PageHeaderProps {
  title: string
  description: string
}

export default function PageHeader({ title, description }: PageHeaderProps) {
  return (
    <header className="mb-6">
      <h1 className="text-xl font-semibold">{title}</h1>
      <p className="mt-1 text-sm text-ink-muted">{description}</p>
    </header>
  )
}
