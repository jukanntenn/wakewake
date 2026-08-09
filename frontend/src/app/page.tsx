import { redirect } from 'next/navigation'

export default function Home() {
  // 路由表（routing-and-guards.md）：/ → /dashboard
  redirect('/dashboard')
}
