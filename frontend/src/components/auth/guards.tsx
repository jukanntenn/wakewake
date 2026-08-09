'use client'

// 声明式守卫（routing-and-guards.md §Guard Architecture）。
// AuthGate 执行器消费 useAuthGuard；PublicRoute/ProtectedRoute/AdminRoute 可组合三层。

import { useEffect, type ReactNode } from 'react'
import { useRouter } from 'next/navigation'
import { useAuthStore } from '@/stores/auth'
import { publicRoute, protectedRoute, adminRoute } from './route-configs'
import { PageSpinner } from './page-spinner'

function useAuthGuard(
  shouldShow: (isAuth: boolean, isAdmin: boolean) => boolean,
  redirectPath: string,
) {
  const { _hasHydrated, user } = useAuthStore()
  const isAuth = user !== null
  const isAdmin = user?.is_superuser === true
  const router = useRouter()

  useEffect(() => {
    if (_hasHydrated && !shouldShow(isAuth, isAdmin)) {
      router.replace(redirectPath)
    }
  }, [_hasHydrated, isAuth, isAdmin, shouldShow, redirectPath, router])

  return { hasHydrated: _hasHydrated, isAuthenticated: isAuth, isAdmin }
}

interface GuardProps {
  children: ReactNode
  shouldShow: (isAuth: boolean, isAdmin: boolean) => boolean
  redirectPath: string
  showSpinnerWhen?: (isAuth: boolean, isAdmin: boolean) => boolean
}

export function AuthGate({ children, shouldShow, redirectPath, showSpinnerWhen }: GuardProps) {
  const { hasHydrated, isAuthenticated, isAdmin } = useAuthGuard(shouldShow, redirectPath)

  if (!hasHydrated) return <PageSpinner />
  if (!shouldShow(isAuthenticated, isAdmin)) {
    return showSpinnerWhen?.(isAuthenticated, isAdmin) ? <PageSpinner /> : null
  }
  return <>{children}</>
}

export function PublicRoute({ children }: { children: ReactNode }) {
  return (
    <AuthGate
      shouldShow={publicRoute.shouldShow}
      redirectPath={publicRoute.redirectPath}
      showSpinnerWhen={publicRoute.showSpinnerWhen}
    >
      {children}
    </AuthGate>
  )
}

export function ProtectedRoute({ children }: { children: ReactNode }) {
  return (
    <AuthGate shouldShow={protectedRoute.shouldShow} redirectPath={protectedRoute.redirectPath}>
      {children}
    </AuthGate>
  )
}

export function AdminRoute({ children }: { children: ReactNode }) {
  return (
    <AuthGate shouldShow={adminRoute.shouldShow} redirectPath={adminRoute.redirectPath}>
      {children}
    </AuthGate>
  )
}
