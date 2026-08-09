// (dashboard) 路由组：ProtectedRoute 守卫 + 侧边栏 header（routing-and-guards.md）。
import { ProtectedRoute } from '@/components/auth/guards'
import { DashboardShell } from '@/components/layout/dashboard-shell'

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <ProtectedRoute>
      <DashboardShell>{children}</DashboardShell>
    </ProtectedRoute>
  )
}
