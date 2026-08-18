import { NavLink } from 'react-router-dom'

const NAV = [
  { to: '/', label: 'Dashboard', end: true },
  { to: '/accounts', label: 'Accounts', end: false },
  { to: '/providers', label: 'Providers', end: false },
  { to: '/relay', label: 'Relay', end: false },
  { to: '/router', label: 'Router', end: false },
  { to: '/settings', label: 'Settings', end: false },
] as const

export default function Sidebar() {
  return (
    <nav
      aria-label="Primary"
      className="w-56 shrink-0 border-r border-border-subtle bg-surface-raised p-4"
    >
      <div className="mb-6 px-2">
        <p className="text-sm font-semibold">Coding Agent Manager</p>
        <p className="text-xs text-ink-muted">v0.1.0 — pre-alpha</p>
      </div>
      <ul className="space-y-1">
        {NAV.map((item) => (
          <li key={item.to}>
            <NavLink
              to={item.to}
              end={item.end}
              className={({ isActive }) =>
                [
                  'block rounded-md px-3 py-2 text-sm',
                  isActive
                    ? 'bg-accent/15 font-medium text-accent'
                    : 'text-ink-muted hover:bg-black/5 dark:hover:bg-white/5',
                ].join(' ')
              }
            >
              {item.label}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  )
}
