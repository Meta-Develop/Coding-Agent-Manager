import { useId, useState, type FormEvent } from 'react'
import type { ProviderDescriptor } from '@/types'

const ACCOUNT_ID_PATTERN = /^[A-Za-z0-9_-]{1,128}$/
const ACCOUNT_ID_MAX_LENGTH = 128

const controlFocus =
  'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent'

/**
 * Adds a stored account for one adapter. The name is the account id, so the
 * allowed characters are on screen before submit. Provider-specific copy
 * describes the native credential ingress without ever accepting a secret
 * in this webview.
 */
export default function AddAccountForm({
  provider,
  disabled,
  onAdd,
}: {
  provider: ProviderDescriptor
  disabled: boolean
  onAdd: (accountId: string) => Promise<boolean>
}) {
  const id = useId()
  const nameId = `${id}-name`
  const rulesId = `${id}-rules`
  const explanationId = `${id}-explanation`
  const errorId = `${id}-error`
  const [name, setName] = useState('')
  const [validation, setValidation] = useState<string | null>(null)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const accountId = name.trim()
    const problem = accountIdProblem(accountId, provider.id)
    if (problem !== null) {
      setValidation(problem)
      return
    }
    const added = await onAdd(accountId)
    if (added) {
      setName('')
      setValidation(null)
    }
  }

  const describedBy = [
    rulesId,
    explanationId,
    validation !== null ? errorId : null,
  ]
    .filter((value): value is string => value !== null)
    .join(' ')

  return (
    <form
      className="mt-4 rounded-lg border border-border-subtle bg-surface-raised p-4"
      onSubmit={(event) => {
        void handleSubmit(event)
      }}
    >
      <div className="flex flex-wrap items-end gap-3">
        <div className="min-w-[12rem] flex-1">
          <label htmlFor={nameId} className="block text-sm font-medium">
            Account name
          </label>
          <input
            id={nameId}
            type="text"
            name="accountName"
            value={name}
            maxLength={ACCOUNT_ID_MAX_LENGTH}
            autoComplete="off"
            spellCheck={false}
            disabled={disabled}
            aria-invalid={validation !== null}
            aria-describedby={describedBy}
            onChange={(event) => {
              setName(event.target.value)
              if (validation !== null) {
                setValidation(null)
              }
            }}
            className={`mt-1 w-full rounded-md border border-border-subtle bg-surface px-3 py-1.5 text-sm text-ink disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`}
          />
        </div>
        <button
          type="submit"
          disabled={disabled}
          className={`rounded-md border border-accent bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent disabled:cursor-not-allowed disabled:opacity-50 ${controlFocus}`}
        >
          Add account to {provider.displayName}
        </button>
      </div>
      <p id={rulesId} className="mt-2 text-sm text-ink-muted">
        The name becomes the account&apos;s id. Use letters, digits,{' '}
        <code className="font-mono">-</code> and{' '}
        <code className="font-mono">_</code>, at most {ACCOUNT_ID_MAX_LENGTH}{' '}
        characters.
      </p>
      <p id={explanationId} className="mt-2 text-sm text-ink-muted">
        {addExplanation(provider)}
      </p>
      {validation !== null && (
        <p id={errorId} className="mt-2 text-sm" role="alert">
          {validation}
        </p>
      )}
    </form>
  )
}

function addExplanation(provider: ProviderDescriptor) {
  if (provider.id === 'gemini-cli') {
    return (
      <>
        This imports <code className="font-mono">GEMINI_API_KEY</code> from this
        application&apos;s native parent process into CredentialStore. The key
        is never typed into or returned to this webview. Start or restart this
        application with the variable set; restart it again with a different
        source key before adding another account. Only Gemini API-key accounts
        are supported here, not Google OAuth accounts.
      </>
    )
  }
  if (provider.id === 'grok-cli') {
    return (
      <>
        Sign-in runs in {provider.displayName} itself, in the terminal that
        launched this application. The vendor writes the credential into a
        retained, isolated home for this account. This window does not host or
        cancel that prompt, and leaving this page does not cancel it.
      </>
    )
  }
  return (
    <>
      Sign-in runs in {provider.displayName} itself, in the terminal that
      launched this application. This window does not host the prompt. Adding an
      account does not return until that sign-in finishes or fails. Closing this
      window or leaving this page does not cancel that sign-in, and this
      application cannot cancel it either. A window-hosted sign-in is not built
      yet.
    </>
  )
}

/** Matches `account_id_is_safe` plus the Codex live-slot reservation. */
const CODEX_LIVE_SLOT_ID = 'codex-cli-on-disk'

function accountIdProblem(
  accountId: string,
  providerId: string,
): string | null {
  if (accountId === '') {
    return 'Enter an account name.'
  }
  if (!ACCOUNT_ID_PATTERN.test(accountId)) {
    return 'Use only letters, digits, `-` and `_`, at most 128 characters.'
  }
  if (providerId === 'codex-cli' && accountId === CODEX_LIVE_SLOT_ID) {
    return `\`${accountId}\` is reserved for the live on-disk Codex identity; choose a different name.`
  }
  return null
}
