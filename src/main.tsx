import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createHashRouter, RouterProvider } from 'react-router-dom'
import App from '@/App'
import Dashboard from '@/pages/Dashboard'
import Accounts from '@/pages/Accounts'
import Providers from '@/pages/Providers'
import Relay from '@/pages/Relay'
import RouterRules from '@/pages/RouterRules'
import Settings from '@/pages/Settings'
import '@/index.css'

// Hash routing keeps deep links working inside the Tauri webview without a
// server-side rewrite rule.
const router = createHashRouter([
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
])

const container = document.getElementById('root')
if (!container) {
  throw new Error('root container is missing from index.html')
}

createRoot(container).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
