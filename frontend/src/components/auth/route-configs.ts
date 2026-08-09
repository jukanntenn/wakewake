// 纯函数守卫配置（routing-and-guards.md §route-configs.ts）。
// 声明每类路由的判定逻辑，与 React 解耦，易测。

export const publicRoute = {
  shouldShow: (isAuth: boolean) => !isAuth,
  redirectPath: '/dashboard',
  showSpinnerWhen: (isAuth: boolean) => isAuth, // 已认证用户访问 /login 时显示 spinner 直到跳转
}

export const protectedRoute = {
  shouldShow: (isAuth: boolean) => isAuth,
  redirectPath: '/login',
}

export const adminRoute = {
  shouldShow: (isAuth: boolean, isAdmin: boolean) => isAuth && isAdmin,
  redirectPath: '/dashboard', // 非 admin 跳回 dashboard
}
