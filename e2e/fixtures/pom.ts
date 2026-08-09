/**
 * 三层 fixture 继承链 第 1 层：POM fixture（e2e.md §3.4）。
 *
 * @playwright/test test (base test object)
 *    ↓ extend<PomFixtures>（pom.ts：注入 Page Object）
 */
import { test as pwBase, expect } from '@playwright/test'
import type {
  LoginPage,
  RegisterPage,
  DashboardPage,
  DevicesPage,
  AgentsPage,
  WakesPage,
  IntegrationsPage,
  SettingsPage,
  AdminUsersPage,
} from '../pages'
import {
  LoginPage as LoginPageImpl,
  RegisterPage as RegisterPageImpl,
  DashboardPage as DashboardPageImpl,
  DevicesPage as DevicesPageImpl,
  AgentsPage as AgentsPageImpl,
  WakesPage as WakesPageImpl,
  IntegrationsPage as IntegrationsPageImpl,
  SettingsPage as SettingsPageImpl,
  AdminUsersPage as AdminUsersPageImpl,
} from '../pages'

export const base = pwBase.extend<{
  loginPage: LoginPage
  registerPage: RegisterPage
  dashboardPage: DashboardPage
  devicesPage: DevicesPage
  agentsPage: AgentsPage
  wakesPage: WakesPage
  integrationsPage: IntegrationsPage
  settingsPage: SettingsPage
  adminUsersPage: AdminUsersPage
}>({
  loginPage: async ({ page }, use) => {
    await use(new LoginPageImpl(page))
  },
  registerPage: async ({ page }, use) => {
    await use(new RegisterPageImpl(page))
  },
  dashboardPage: async ({ page }, use) => {
    await use(new DashboardPageImpl(page))
  },
  devicesPage: async ({ page }, use) => {
    await use(new DevicesPageImpl(page))
  },
  agentsPage: async ({ page }, use) => {
    await use(new AgentsPageImpl(page))
  },
  wakesPage: async ({ page }, use) => {
    await use(new WakesPageImpl(page))
  },
  integrationsPage: async ({ page }, use) => {
    await use(new IntegrationsPageImpl(page))
  },
  settingsPage: async ({ page }, use) => {
    await use(new SettingsPageImpl(page))
  },
  adminUsersPage: async ({ page }, use) => {
    await use(new AdminUsersPageImpl(page))
  },
})

// 重导出 expect 供下游 fixture链 + specs 用
export { expect }
export type { LoginPage, RegisterPage, DashboardPage }
