import { useId, type KeyboardEvent, type ReactNode } from 'react'
import type { Account } from '@/types'

const controlFocus =
  'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent'

const buttonClass = `shrink-0 whitespace-nowrap rounded-md border border-border-subtle px-3 py-1.5 text-sm text-ink disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`

const confirmClass = `whitespace-nowrap rounded-md border border-accent bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`

export type PendingKind = 'switch' | 'delete'

/**
 * Per-row switch and delete, gated by the adapter's capabilities and
 * whether this application holds a stored copy of the row. Incomplete
 * rows can be deleted but never switched. Confirm and Cancel sit in a
 * following table row rather than in a dialog so they stay in the tab
 * order without a focus trap (`NFR-6`).
 */
export default function AccountActions({
  account,
  canSwitch,
  canDelete,
  disabled,
  onRequest,
}: {
  account: Account
  canSwitch: boolean
  canDelete: boolean
  disabled: boolean
  onRequest: (kind: PendingKind) => void
}) {
  const name = accountDisplayName(account)

  if (!canSwitch && !canDelete) {
    return null
  }

  return (
    <div className="flex w-max flex-nowrap items-center gap-2">
      {canSwitch && (
        <button
          type="button"
          disabled={disabled}
          className={buttonClass}
          onClick={() => onRequest('switch')}
        >
          Switch to {name}
        </button>
      )}
      {canDelete && (
        <button
          type="button"
          disabled={disabled}
          className={buttonClass}
          onClick={() => onRequest('delete')}
        >
          Delete {name}
        </button>
      )}
    </div>
  )
}

/**
 * Inline confirm/cancel for a pending switch or delete. Rendered as a
 * full-width row under the account it refers to so the sentence is not
 * squeezed into the actions column.
 */
export function Confirmation({
  id,
  label,
  confirmLabel,
  cancelLabel,
  disabled,
  onCancel,
  onConfirm,
  children,
}: {
  id?: string
  label: string
  confirmLabel: string
  cancelLabel: string
  disabled: boolean
  onCancel: () => void
  onConfirm: () => void
  children: ReactNode
}) {
  const textId = useId()

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onCancel()
    }
  }

  return (
    <div
      id={id}
      role="group"
      aria-label={label}
      className="space-y-2"
      onKeyDown={handleKeyDown}
    >
      <p id={textId} className="text-sm text-ink">
        {children}
      </p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          autoFocus
          disabled={disabled}
          className={confirmClass}
          aria-label={confirmLabel}
          aria-describedby={textId}
          onClick={onConfirm}
        >
          Confirm
        </button>
        <button
          type="button"
          className={buttonClass}
          aria-label={cancelLabel}
          onClick={onCancel}
        >
          Cancel
        </button>
      </div>
    </div>
  )
}

export function accountDisplayName(account: Account): string {
  const label = account.label.trim()
  return label === '' ? account.id : label
}
