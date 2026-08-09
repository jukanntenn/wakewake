/**
 * 统一导出 test + expect（e2e.md §3.4 fixtures/index.ts）。
 *
 * 三层 fixture 继承链：
 *   @playwright/test base
 *      ↓ extend<PomFixtures>（pom.ts）
 *      ↓ extend<DataFixtures>（data.ts）
 *      ↓ extend<AuthFixtures>（auth.ts）
 *      ↓
 *   fixtures/index.ts → export { test, expect }
 *
 * 所有 spec 用 `import { test, expect } from '../fixtures'`。
 */
export { base as test, expect } from './auth'
