import { AdminRoute } from '@/components/auth/guards'

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return <AdminRoute>{children}</AdminRoute>
}
