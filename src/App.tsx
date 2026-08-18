import { Outlet } from 'react-router-dom'
import Sidebar from '@/components/Sidebar'

export default function App() {
  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="flex-1 overflow-y-auto p-8">
        <Outlet />
      </main>
    </div>
  )
}
