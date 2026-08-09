import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

// User 含 is_superuser（routing-and-guards.md §isAdmin 来源，DB schema users.is_superuser）。
export interface User {
  id: number
  email: string
  is_superuser: boolean
  email_verified?: boolean
}

interface AuthState {
  // access token 不 persist（内存，降 XSS 偷取窗口，authentication.md §九）
  token: string | null
  refreshToken: string | null
  user: User | null
  _hasHydrated: boolean
  setAuth: (token: string, user: User, refreshToken?: string) => void
  setToken: (token: string) => void
  logout: () => void
  setHasHydrated: (state: boolean) => void
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: null,
      refreshToken: null,
      user: null,
      _hasHydrated: false,
      setAuth: (token, user, refreshToken) =>
        set({ token, user, refreshToken: refreshToken || null }),
      setToken: (token) => set({ token }),
      logout: () => set({ token: null, refreshToken: null, user: null }),
      setHasHydrated: (state) => set({ _hasHydrated: state }),
    }),
    {
      name: 'wakewake-auth',
      storage: createJSONStorage(() => localStorage),
      // 只 persist refreshToken + user（access 不 persist，authentication.md §九）
      partialize: (state) => ({ refreshToken: state.refreshToken, user: state.user }),
      onRehydrateStorage: () => (state) => {
        state?.setHasHydrated(true)
      },
    },
  ),
)

// 派生 selector：isAdmin = user.is_superuser（routing-and-guards.md）
export const useIsAdmin = (): boolean => useAuthStore((s) => s.user?.is_superuser === true)
