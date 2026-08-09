import { vi, afterEach } from 'vitest'
import * as matchers from '@testing-library/jest-dom/matchers'
import { expect } from 'vitest'
import { cleanup } from '@testing-library/react'

// Extend expect with jest-dom matchers
expect.extend(matchers)

// 每个测试后自动清理 DOM（防跨测试元素残留导致 multiple-found 错误）
afterEach(() => {
  cleanup()
})

// 全局 Mock
global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}))

// Mock window.matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})
