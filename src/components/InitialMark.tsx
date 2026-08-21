/** Abstract initial badge. Not a vendor mark. */
export default function InitialMark({ name }: { name: string }) {
  const initial = (name.trim().charAt(0) || '?').toLocaleUpperCase()
  return (
    <span
      aria-hidden="true"
      className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-border-subtle bg-surface text-xs font-semibold text-ink shadow-control"
    >
      {initial}
    </span>
  )
}
