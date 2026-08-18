interface NotImplementedProps {
  /** Requirement id from `docs/SPEC.md`, e.g. `FR-6`. */
  requirement: string
  children: React.ReactNode
}

/**
 * Placeholder for a screen whose behaviour is specified but not yet built.
 * Keeping the requirement id visible makes unfinished surface area auditable.
 */
export default function NotImplemented({
  requirement,
  children,
}: NotImplementedProps) {
  return (
    <div className="rounded-lg border border-dashed border-border-subtle p-6">
      <p className="text-xs font-mono uppercase tracking-wide text-ink-muted">
        Not implemented — {requirement}
      </p>
      <div className="mt-2 text-sm text-ink-muted">{children}</div>
    </div>
  )
}
