import { Outlet } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'
import { AccountMutationProvider, MutationNotice } from '@/lib/accountMutation'

export default function App() {
  return (
    <AccountMutationProvider>
      <div className="flex h-full">
        <Sidebar />
        <main className="flex-1 overflow-y-auto p-8">
          <MutationNotice />
          <Outlet />
        </main>
      </div>
    </AccountMutationProvider>
  )
}
