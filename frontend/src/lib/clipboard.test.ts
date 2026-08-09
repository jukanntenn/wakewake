import { describe, it, expect, vi, beforeEach } from 'vitest'

// 顶层 mock copy-to-clipboard：用可变实现控制返回值（v4 起 copy 返回 Promise<boolean>）
const copyMock = vi.fn<(s: string) => Promise<boolean>>()
vi.mock('copy-to-clipboard', () => ({ default: copyMock }))

describe('lib/clipboard — copyText', () => {
  beforeEach(() => {
    copyMock.mockReset()
  })

  it('forwards text to copy-to-clipboard and returns true on success', async () => {
    copyMock.mockResolvedValue(true)
    const { copyText } = await import('./clipboard')
    const result = await copyText('hello')
    expect(copyMock).toHaveBeenCalledWith('hello')
    expect(result).toBe(true)
  })

  it('returns false when underlying copy returns false (降级失败)', async () => {
    copyMock.mockResolvedValue(false)
    const { copyText } = await import('./clipboard')
    expect(await copyText('x')).toBe(false)
  })
})
