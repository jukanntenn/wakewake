import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    // 并行 worker 上限对齐 CI（ubuntu-latest 4 核）：本机默认吃满全部核时，
    // 高负载下 jsdom 用例会撞超时（门禁偶发红）。
    maxWorkers: 4,
    // admin 表格页（桌面表格 + 移动卡片双渲染）单用例本机冷跑 ~4.4s、套件
    // 并发时 >5s，默认 5000ms 会把慢机器上的通过用例判死；断言同步、无等待
    // 循环，放宽上限不改语义。
    testTimeout: 15_000,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html', 'cobertura', 'junit'],
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
})
