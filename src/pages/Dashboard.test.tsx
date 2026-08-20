import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import Dashboard from '@/pages/Dashboard'
import { listAccounts, listProviders, listQuota } from '@/lib/tauri'
import type {
  ProviderAccountList,
  ProviderDescriptor,
  ProviderQuotaList,
} from '@/types'

vi.mock('@/lib/tauri', () => ({
  listAccounts: vi.fn(),
  listProviders: vi.fn(),
  listQuota: vi.fn(),
}))

describe('Dashboard quota visibility', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(listProviders).mockResolvedValue(PROVIDERS)
    vi.mocked(listAccounts).mockResolvedValue(
      PROVIDERS.map(({ id }) => listing(id)),
    )
    vi.mocked(listQuota).mockResolvedValue(
      PROVIDERS.map(({ id }) => noSignal(id)),
    )
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('renders an explicit no-signal state for every registered provider', async () => {
    render(<Dashboard />)

    for (const provider of PROVIDERS) {
      const card = await screen.findByRole('article', {
        name: `${provider.displayName} quota`,
      })
      expect(within(card).getByText('No quota signal available')).toBeVisible()
      expect(within(card).getByText(provider.maturity)).toBeVisible()
      expect(within(card).getByText('Unavailable')).toBeVisible()
      expect(within(card).queryByText(/% remaining/)).not.toBeInTheDocument()
    }
  })

  it('switches between accessible grid and list views', async () => {
    const user = userEvent.setup()
    render(<Dashboard />)

    expect(
      await screen.findByRole('list', { name: 'Quota grid' }),
    ).toBeVisible()
    expect(screen.getByRole('button', { name: 'Grid' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByRole('button', { name: 'List' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )

    await user.click(screen.getByRole('button', { name: 'List' }))

    expect(screen.getByRole('list', { name: 'Quota list' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'List' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getAllByRole('article')).toHaveLength(PROVIDERS.length)
  })

  it('shows sourced remaining quota, plan, window, reset, source, and capture age', async () => {
    const provider = CLAUDE_PROVIDER
    const capturedAt = '2026-08-20T10:00:00Z'
    const resetsAt = '2026-08-20T15:00:00Z'
    vi.spyOn(Date, 'now').mockReturnValue(
      new Date('2026-08-20T12:00:00Z').getTime(),
    )
    vi.mocked(listProviders).mockResolvedValue([provider])
    vi.mocked(listAccounts).mockResolvedValue([listing(provider.id)])
    vi.mocked(listQuota).mockResolvedValue([
      {
        providerId: provider.id,
        planLabel: 'Pro',
        outcome: { kind: 'available' },
        snapshots: [
          {
            accountId: 'work',
            model: 'claude-sonnet',
            utilization: 0.25,
            windowLabel: '5 hours',
            resetsAt,
            capturedAt,
            source: 'local-file',
          },
        ],
      },
    ])

    render(<Dashboard />)

    const card = await screen.findByRole('article', {
      name: `${provider.displayName} quota`,
    })
    expect(within(card).getByText('75% remaining')).toBeVisible()
    expect(within(card).getByText('Pro')).toBeVisible()
    expect(within(card).getByText('5 hours')).toBeVisible()
    expect(within(card).getByText('local-file')).toBeVisible()
    expect(within(card).getByText('2 hours ago')).toHaveAttribute(
      'dateTime',
      capturedAt,
    )
    expect(within(card).getByTitle(resetsAt)).toHaveAttribute(
      'dateTime',
      resetsAt,
    )
  })

  it('renders collection failures distinctly from no-signal states', async () => {
    const failedProvider = CLAUDE_PROVIDER
    const emptyProvider = CODEX_PROVIDER
    vi.mocked(listProviders).mockResolvedValue([failedProvider, emptyProvider])
    vi.mocked(listAccounts).mockResolvedValue([
      listing(failedProvider.id),
      listing(emptyProvider.id),
    ])
    vi.mocked(listQuota).mockResolvedValue([
      {
        providerId: failedProvider.id,
        planLabel: null,
        snapshots: [],
        outcome: {
          kind: 'failed',
          error: {
            kind: 'config-read',
            path: null,
            message: 'Quota cache is unreadable',
          },
        },
      },
      noSignal(emptyProvider.id),
    ])

    render(<Dashboard />)

    const failedCard = await screen.findByRole('article', {
      name: `${failedProvider.displayName} quota`,
    })
    expect(within(failedCard).getByRole('alert')).toHaveTextContent(
      'Quota collection failed: Quota cache is unreadable',
    )
    expect(
      within(failedCard).queryByText('No quota signal available'),
    ).not.toBeInTheDocument()

    const emptyCard = screen.getByRole('article', {
      name: `${emptyProvider.displayName} quota`,
    })
    expect(
      within(emptyCard).getByText('No quota signal available'),
    ).toBeVisible()
    expect(within(emptyCard).queryByRole('alert')).not.toBeInTheDocument()
  })

  it('treats a missing provider result as an error', async () => {
    vi.mocked(listProviders).mockResolvedValue([CLAUDE_PROVIDER])
    vi.mocked(listAccounts).mockResolvedValue([listing(CLAUDE_PROVIDER.id)])
    vi.mocked(listQuota).mockResolvedValue([])

    render(<Dashboard />)

    const card = await screen.findByRole('article', {
      name: `${CLAUDE_PROVIDER.displayName} quota`,
    })
    expect(within(card).getByRole('alert')).toHaveTextContent(
      'Quota result missing for this provider.',
    )
    expect(
      within(card).queryByText('No quota signal available'),
    ).not.toBeInTheDocument()
  })

  it('keeps the provider summary when quota loading rejects', async () => {
    vi.mocked(listProviders).mockResolvedValue([CLAUDE_PROVIDER])
    vi.mocked(listAccounts).mockResolvedValue([listing(CLAUDE_PROVIDER.id)])
    vi.mocked(listQuota).mockImplementation(() => {
      throw new Error('quota command unavailable')
    })

    render(<Dashboard />)

    expect(
      await screen.findByRole('row', {
        name: /Providers detected 1 of 1 — Claude Code/,
      }),
    ).toBeVisible()
    const card = screen.getByRole('article', {
      name: `${CLAUDE_PROVIDER.displayName} quota`,
    })
    expect(within(card).getByRole('alert')).toHaveTextContent(
      'Quota collection failed: Error: quota command unavailable',
    )
  })
})

const CLAUDE_PROVIDER = provider('claude-code', 'Claude Code')
const CODEX_PROVIDER = provider('codex-cli', 'Codex CLI')
const PROVIDERS: ProviderDescriptor[] = [
  CLAUDE_PROVIDER,
  CODEX_PROVIDER,
  provider('cursor', 'Cursor', 'planned'),
  provider('grok-cli', 'Grok CLI'),
  provider('gemini-cli', 'Gemini CLI'),
]

function provider(
  id: string,
  displayName: string,
  maturity: ProviderDescriptor['maturity'] = 'experimental',
): ProviderDescriptor {
  return {
    id,
    displayName,
    vendor: 'Vendor',
    authKinds: ['oauth'],
    maturity,
    installState: 'installed',
    capabilities: [],
  }
}

function listing(providerId: string): ProviderAccountList {
  return {
    providerId,
    accounts: [],
    outcome: { kind: 'listed' },
  }
}

function noSignal(providerId: string): ProviderQuotaList {
  return {
    providerId,
    planLabel: null,
    snapshots: [],
    outcome: { kind: 'no-signal' },
  }
}
