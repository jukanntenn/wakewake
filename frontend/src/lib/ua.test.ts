import { describe, it, expect } from 'vitest'
import { summarizeUA } from './ua'

describe('summarizeUA (§11.C.3)', () => {
  it('returns Unknown for null', () => {
    expect(summarizeUA(null)).toBe('Unknown')
  })

  it('detects curl', () => {
    expect(summarizeUA('curl/7.68.0')).toBe('curl')
  })

  it('detects wget', () => {
    expect(summarizeUA('wget/1.20')).toBe('wget')
  })

  it('detects Chrome on macOS', () => {
    const ua =
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    expect(summarizeUA(ua)).toBe('Chrome on macOS')
  })

  it('detects Edge (not Chrome) on Windows', () => {
    const ua =
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0'
    expect(summarizeUA(ua)).toBe('Edge on Windows')
  })

  it('detects Firefox on Linux', () => {
    const ua = 'Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0'
    expect(summarizeUA(ua)).toBe('Firefox on Linux')
  })

  it('detects Safari (not Chrome)', () => {
    const ua =
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15'
    expect(summarizeUA(ua)).toBe('Safari on macOS')
  })

  it('truncates unknown long UA', () => {
    const ua = 'SomeUnknownBot/1.0 with a very long description string exceeding forty chars total'
    const result = summarizeUA(ua)
    expect(result.length).toBeLessThanOrEqual(40)
    expect(result.endsWith('...')).toBe(true)
  })

  it('keeps short unknown UA as-is', () => {
    expect(summarizeUA('Bot/1.0')).toBe('Bot/1.0')
  })
})
