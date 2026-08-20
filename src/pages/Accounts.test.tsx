import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import { describe, expect, it, vi } from 'vitest'
import App from '@/App'
import Dashboard from '@/pages/Dashboard'
import Accounts from '@/pages/Accounts'
import Providers from '@/pages/Providers'
import Relay from '@/pages/Relay'
import RouterRules from '@/pages/RouterRules'
import Settings from '@/pages/Settings'
import type { Account, ProviderAccountList, ProviderDescriptor } from '@/types'

describe('Accounts page', () => {
  it('shows no Add, Switch, or Delete when the provider advertises nothing', async () => {
    stubInvoke({
      providers: [
        provider({
          id: 'cursor',
          displayName: 'Cursor',
          vendor: 'Anysphere',
          capabilities: [],
        }),
      ],
      listings: [
        listing('cursor', {
          accounts: [
            account({
              id: 'work',
              providerId: 'cursor',
              label: 'Work',
              isStored: true,
            }),
          ],
        }),
      ],
    })
    renderApp()

    await screen.findByRole('heading', { name: 'Cursor' })
    expect(
      screen.queryByRole('button', { name: /Add account/i }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: /Switch/i }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: /Delete/i }),
    ).not.toBeInTheDocument()
  })

  it('offers Switch and Delete only on stored complete rows', async () => {
    stubInvoke({
      listings: [
        listing('codex-cli', {
          accounts: [
            account({
              id: 'codex-cli-on-disk',
              label: 'Codex CLI',
              maskedIdentity: '****ab12',
              isActive: true,
              isStored: false,
            }),
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****cd34',
              isStored: true,
            }),
            account({
              id: 'unfinished',
              label: 'unfinished',
              maskedIdentity: null,
              authKind: 'unknown',
              isStored: true,
              isIncomplete: true,
            }),
          ],
        }),
      ],
    })
    renderApp()

    await screen.findByRole('table')

    expect(
      screen.queryByRole('button', { name: 'Switch to Codex CLI' }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Delete Codex CLI' }),
    ).not.toBeInTheDocument()

    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Delete Work' }),
    ).toBeInTheDocument()

    expect(
      screen.queryByRole('button', { name: 'Switch to unfinished' }),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Delete unfinished' }),
    ).toBeInTheDocument()

    const incompleteRow = screen.getByRole('row', { name: /unfinished/i })
    expect(
      within(incompleteRow).getByText('No usable credential'),
    ).toBeInTheDocument()
    // Visible status is the one-word label so the cell stays on one line;
    // the full explanation is on title (and identity already says the
    // credential is unusable).
    const incompleteStatus = within(incompleteRow).getByTitle(
      'Incomplete — sign-in never finished',
    )
    expect(incompleteStatus).toHaveTextContent(/^Incomplete$/)
  })

  it('renders the table and the damaged-file error for listed-with-error', async () => {
    stubInvoke({
      listings: [
        listing('codex-cli', {
          outcome: {
            kind: 'listed-with-error',
            error: {
              kind: 'config-read',
              path: '/tmp/cam-test/auth.json',
              message:
                'configuration for `codex-cli` could not be read: /tmp/cam-test/auth.json is not valid JSON',
            },
          },
          accounts: [
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****ab12',
              isStored: true,
            }),
          ],
        }),
      ],
    })
    renderApp()

    await screen.findByRole('table')
    expect(screen.getByText(/own login file is damaged/i)).toBeInTheDocument()
    // The adapter path is an inline <code>, so the sentence is no longer
    // one text node. The notice still carries the same error text.
    expect(
      screen.getByText(/own login file is damaged/i).parentElement,
    ).toHaveTextContent(/auth\.json is not valid JSON/)
    expect(screen.queryByText(/Looking failed/i)).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeInTheDocument()
  })

  it('renders the error and no table for a failed listing', async () => {
    stubInvoke({
      listings: [
        listing('codex-cli', {
          outcome: {
            kind: 'failed',
            error: {
              kind: 'config-read',
              path: '/tmp/cam-test/auth.json',
              message:
                'configuration for `codex-cli` could not be read: /tmp/cam-test/auth.json is not valid JSON',
            },
          },
          accounts: [
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****ab12',
              isStored: true,
            }),
          ],
        }),
      ],
    })
    renderApp()

    await screen.findByText(/Looking failed:/)
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Switch to Work' }),
    ).not.toBeInTheDocument()
  })

  it('asks before switch, cancels, confirms, and dismisses on Escape', async () => {
    const user = userEvent.setup()
    const activate = vi.fn(async () => undefined)
    stubInvoke({
      listings: [
        listing('codex-cli', {
          accounts: [
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****ab12',
              isStored: true,
            }),
          ],
        }),
      ],
      activate,
    })
    renderApp()

    await screen.findByRole('button', { name: 'Switch to Work' })

    await user.click(screen.getByRole('button', { name: 'Switch to Work' }))
    expect(screen.getByText(/Switch Codex CLI to Work\?/)).toBeInTheDocument()
    expect(activate).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'Cancel switch' }))
    expect(
      screen.queryByText(/Switch Codex CLI to Work\?/),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Switch to Work' }))
    await user.keyboard('{Escape}')
    expect(
      screen.queryByText(/Switch Codex CLI to Work\?/),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Switch to Work' }))
    await user.click(
      screen.getByRole('button', { name: 'Confirm switch to Work' }),
    )
    expect(activate).toHaveBeenCalledTimes(1)
    expect(activate).toHaveBeenCalledWith('codex-cli', 'work')
  })

  it('shows a failed mutation as an alert and keeps the previous listing', async () => {
    const user = userEvent.setup()
    stubInvoke({
      listings: [
        listing('codex-cli', {
          accounts: [
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****ab12',
              isStored: true,
            }),
            account({
              id: 'personal',
              label: 'Personal',
              maskedIdentity: '****cd34',
              isActive: true,
              isStored: true,
            }),
          ],
        }),
      ],
      activate: async () => {
        throw 'Codex CLI appears to be running (process name `codex`). Close the Codex CLI before switching.'
      },
    })
    renderApp()

    await screen.findByRole('button', { name: 'Switch to Work' })
    const listsBefore = callsOf('list_accounts').length

    await user.click(screen.getByRole('button', { name: 'Switch to Work' }))
    await user.click(
      screen.getByRole('button', { name: 'Confirm switch to Work' }),
    )

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not switch Codex CLI to Work:')
    expect(alert).toHaveTextContent(
      'Codex CLI appears to be running (process name `codex`). Close the Codex CLI before switching.',
    )
    expect(callsOf('list_accounts')).toHaveLength(listsBefore)
    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(
      within(screen.getByRole('row', { name: /Personal/ })).getByText('Active'),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeInTheDocument()
  })

  it('re-fetches the listing after a successful mutation instead of editing it in place', async () => {
    const user = userEvent.setup()
    let listings: ProviderAccountList[] = [
      listing('codex-cli', {
        accounts: [
          account({
            id: 'work',
            label: 'Work',
            maskedIdentity: '****ab12',
            isStored: true,
          }),
          account({
            id: 'personal',
            label: 'Personal',
            maskedIdentity: '****cd34',
            isActive: true,
            isStored: true,
          }),
        ],
      }),
    ]
    stubInvoke({
      listings: () => listings,
      activate: async () => {
        listings = [
          listing('codex-cli', {
            accounts: [
              account({
                id: 'work',
                label: 'Work',
                maskedIdentity: '****zz99',
                isActive: true,
                isStored: true,
              }),
              account({
                id: 'personal',
                label: 'Personal',
                maskedIdentity: '****cd34',
                isStored: true,
              }),
            ],
          }),
        ]
      },
    })
    renderApp()

    await screen.findByText('****ab12')
    const listsBefore = callsOf('list_accounts').length

    await user.click(screen.getByRole('button', { name: 'Switch to Work' }))
    await user.click(
      screen.getByRole('button', { name: 'Confirm switch to Work' }),
    )

    expect(await screen.findByText('****zz99')).toBeInTheDocument()
    expect(screen.queryByText('****ab12')).not.toBeInTheDocument()
    expect(
      within(screen.getByRole('row', { name: /Work/ })).getByText('Active'),
    ).toBeInTheDocument()
    expect(callsOf('list_accounts').length).toBeGreaterThan(listsBefore)
  })

  it('disables mutating controls while a mutation is running, including after remount', async () => {
    const user = userEvent.setup()
    let finishAdd!: () => void
    stubInvoke({
      listings: [
        listing('codex-cli', {
          accounts: [
            account({
              id: 'work',
              label: 'Work',
              maskedIdentity: '****ab12',
              isStored: true,
            }),
          ],
        }),
      ],
      add: () =>
        new Promise<void>((resolve) => {
          finishAdd = resolve
        }),
    })
    renderApp()

    await screen.findByRole('textbox', { name: 'Account name' })
    await user.type(
      screen.getByRole('textbox', { name: 'Account name' }),
      'unfinished',
    )
    await user.click(
      screen.getByRole('button', { name: 'Add account to Codex CLI' }),
    )

    expect(
      await screen.findByText(/Adding unfinished to Codex CLI/),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Add account to Codex CLI' }),
    ).toBeDisabled()
    expect(screen.getByRole('textbox', { name: 'Account name' })).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Delete Work' })).toBeDisabled()
    expect(screen.queryByText(/cancell/i)).not.toBeInTheDocument()

    await user.click(screen.getByRole('link', { name: 'Dashboard' }))
    expect(
      screen.getByText(/Adding unfinished to Codex CLI/),
    ).toBeInTheDocument()
    expect(screen.queryByText(/cancell/i)).not.toBeInTheDocument()
    expect(
      screen.queryByRole('textbox', { name: 'Account name' }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('link', { name: 'Accounts' }))
    expect(
      await screen.findByRole('button', { name: 'Add account to Codex CLI' }),
    ).toBeDisabled()
    expect(
      screen.getByText(/Adding unfinished to Codex CLI/),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Switch to Work' }),
    ).toBeDisabled()
    expect(screen.queryByText(/cancell/i)).not.toBeInTheDocument()

    finishAdd()
  })

  it('rejects an id the backend would reject before calling add_account', async () => {
    const user = userEvent.setup()
    const add = vi.fn(async () => undefined)
    stubInvoke({
      listings: [listing('codex-cli', { accounts: [] })],
      add,
    })
    renderApp()

    const name = await screen.findByRole('textbox', { name: 'Account name' })
    const submit = screen.getByRole('button', {
      name: 'Add account to Codex CLI',
    })

    await user.click(submit)
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Enter an account name.',
    )
    expect(add).not.toHaveBeenCalled()

    await user.type(name, 'acct/work')
    await user.click(submit)
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Use only letters, digits, `-` and `_`, at most 128 characters.',
    )
    expect(add).not.toHaveBeenCalled()

    await user.clear(name)
    await user.type(name, 'codex-cli-on-disk')
    await user.click(submit)
    expect(screen.getByRole('alert')).toHaveTextContent(
      '`codex-cli-on-disk` is reserved for the live on-disk Codex identity; choose a different name.',
    )
    expect(add).not.toHaveBeenCalled()
    expect(callsOf('add_account')).toHaveLength(0)
  })
})

function renderApp(path = '/accounts') {
  const router = createMemoryRouter(
    [
      {
        path: '/',
        element: <App />,
        children: [
          { index: true, element: <Dashboard /> },
          { path: 'accounts', element: <Accounts /> },
          { path: 'providers', element: <Providers /> },
          { path: 'relay', element: <Relay /> },
          { path: 'router', element: <RouterRules /> },
          { path: 'settings', element: <Settings /> },
        ],
      },
    ],
    { initialEntries: [path] },
  )
  return render(<RouterProvider router={router} />)
}

function stubInvoke(options: {
  providers?: ProviderDescriptor[]
  listings?: ProviderAccountList[] | (() => ProviderAccountList[])
  add?: (providerId: string, accountId: string) => Promise<void>
  activate?: (providerId: string, accountId: string) => Promise<void>
  remove?: (providerId: string, accountId: string) => Promise<void>
}) {
  const providers = options.providers ?? [provider()]
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    const payload = (args ?? {}) as {
      providerId?: string | null
      accountId?: string
    }
    switch (command) {
      case 'list_providers':
        return providers
      case 'list_accounts':
        return typeof options.listings === 'function'
          ? options.listings()
          : (options.listings ?? [])
      case 'add_account':
        if (options.add) {
          await options.add(payload.providerId ?? '', payload.accountId ?? '')
        }
        return undefined
      case 'activate_account':
        if (options.activate) {
          await options.activate(
            payload.providerId ?? '',
            payload.accountId ?? '',
          )
        }
        return undefined
      case 'delete_account':
        if (options.remove) {
          await options.remove(
            payload.providerId ?? '',
            payload.accountId ?? '',
          )
        }
        return undefined
      default:
        throw new Error(`unexpected command: ${String(command)}`)
    }
  })
}

function callsOf(command: string) {
  return vi.mocked(invoke).mock.calls.filter(([name]) => name === command)
}

function provider(
  partial: Partial<ProviderDescriptor> = {},
): ProviderDescriptor {
  return {
    id: 'codex-cli',
    displayName: 'Codex CLI',
    vendor: 'OpenAI',
    authKinds: ['oauth', 'api-key'],
    maturity: 'experimental',
    installState: 'installed',
    capabilities: ['add-account', 'switch-account', 'delete-account'],
    ...partial,
  }
}

function listing(
  providerId: string,
  partial: Partial<ProviderAccountList> = {},
): ProviderAccountList {
  return {
    providerId,
    accounts: [],
    outcome: { kind: 'listed' },
    ...partial,
  }
}

function account(partial: Partial<Account> & Pick<Account, 'id'>): Account {
  return {
    providerId: 'codex-cli',
    label: partial.id,
    maskedIdentity: '****ab12',
    authKind: 'oauth',
    isActive: false,
    isSelectedForLaunch: false,
    isStored: true,
    isIncomplete: false,
    expiresAt: null,
    ...partial,
  }
}
