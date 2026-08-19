import { useId, type KeyboardEvent, type ReactNode } from 'react'
import type { Account, ProviderDescriptor } from '@/types'

const controlFocus =
  'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent'

const buttonClass = `rounded-md border border-border-subtle px-3 py-1.5 text-sm text-ink disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`

const confirmClass = `rounded-md border border-accent bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`

export type PendingKind = 'switch' | 'delete'

/**
 * Per-row switch and delete, gated by the adapter's capabilities and
 * whether this application holds a stored copy of the row. Incomplete
 * rows can be deleted but never switched. Confirm and Cancel sit on
 * the row rather than in a dialog so they stay in the tab order without
 * a focus trap (`NFR-6`).
 */
export default function AccountActions({
  account,
  provider,
  canSwitch,
  canDelete,
  disabled,
  pending,
  onRequest,
  onCancel,
  onConfirm,
}: {
  account: Account
  provider: ProviderDescriptor
  canSwitch: boolean
  canDelete: boolean
  disabled: boolean
  pending: PendingKind | null
  onRequest: (kind: PendingKind) => void
  onCancel: () => void
  onConfirm: () => void
}) {
  const name = accountDisplayName(account)

  if (pending === 'switch') {
    return (
      <Confirmation
        label={`Confirm switch to ${name}`}
        confirmLabel={`Confirm switch to ${name}`}
        cancelLabel="Cancel switch"
        disabled={disabled}
        onCancel={onCancel}
        onConfirm={onConfirm}
      >
        Switch {provider.displayName} to {name}? This replaces the credential
        file in the tool&apos;s own home, behind a restorable backup.{' '}
        {provider.displayName} must not be running.
      </Confirmation>
    )
  }

  if (pending === 'delete') {
    return (
      <Confirmation
        label={`Confirm deletion of ${name}`}
        confirmLabel={`Confirm deletion of ${name}`}
        cancelLabel="Cancel deletion"
        disabled={disabled}
        onCancel={onCancel}
        onConfirm={onConfirm}
      >
        Forget this application&apos;s stored copy of {name}?{' '}
        {provider.displayName} is not signed out, and its own files are left
        untouched.
      </Confirmation>
    )
  }

  return (
    <div className="flex flex-wrap gap-2">
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

function Confirmation({
  label,
  confirmLabel,
  cancelLabel,
  disabled,
  onCancel,
  onConfirm,
  children,
}: {
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
      role="group"
      aria-label={label}
      className="max-w-md space-y-2"
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
