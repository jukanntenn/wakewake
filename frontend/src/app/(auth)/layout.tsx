// (auth) 路由组：PublicRoute 守卫（routing-and-guards.md §布局层级应用）。
import { PublicRoute } from '@/components/auth/guards'

export default function AuthLayout({ children }: { children: React.ReactNode }) {
  return <PublicRoute>{children}</PublicRoute>
}
