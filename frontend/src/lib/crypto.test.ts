import { describe, it, expect } from 'vitest'
import { encryptWithPublicKey, maskMACAddress, isValidMAC, formatMAC } from './crypto'

describe('crypto utilities', () => {
  describe('isValidMAC', () => {
    it('should validate correct MAC addresses with colons', () => {
      expect(isValidMAC('AA:BB:CC:DD:EE:FF')).toBe(true)
      expect(isValidMAC('00:11:22:33:44:55')).toBe(true)
    })

    it('should validate correct MAC addresses with hyphens', () => {
      expect(isValidMAC('AA-BB-CC-DD-EE-FF')).toBe(true)
    })

    it('should validate lowercase MAC addresses', () => {
      expect(isValidMAC('aa:bb:cc:dd:ee:ff')).toBe(true)
    })

    it('should reject invalid MAC addresses', () => {
      expect(isValidMAC('AA:BB:CC:DD:EE')).toBe(false) // Too short
      expect(isValidMAC('AA:BB:CC:DD:EE:FF:GG')).toBe(false) // Too long
      expect(isValidMAC('invalid')).toBe(false)
      expect(isValidMAC('')).toBe(false)
    })
  })

  describe('formatMAC', () => {
    it('should format MAC address with colons to uppercase', () => {
      expect(formatMAC('aa:bb:cc:dd:ee:ff')).toBe('AA:BB:CC:DD:EE:FF')
    })

    it('should format MAC address with hyphens to colons', () => {
      expect(formatMAC('AA-BB-CC-DD-EE-FF')).toBe('AA:BB:CC:DD:EE:FF')
    })

    it('should handle mixed separators', () => {
      expect(formatMAC('AA:BB-CC:DD-EE:FF')).toBe('AA:BB:CC:DD:EE:FF')
    })
  })

  describe('maskMACAddress', () => {
    it('should mask MAC address correctly with colons', () => {
      expect(maskMACAddress('AA:BB:CC:DD:EE:FF')).toBe('AA:**:**:**:**:FF')
    })

    it('should mask MAC address correctly with hyphens', () => {
      expect(maskMACAddress('AA-BB-CC-DD-EE-FF')).toBe('AA:**:**:**:**:FF')
    })

    it('should convert to uppercase', () => {
      expect(maskMACAddress('aa:bb:cc:dd:ee:ff')).toBe('AA:**:**:**:**:FF')
    })

    it('should throw error for invalid MAC format', () => {
      expect(() => maskMACAddress('invalid')).toThrow('Invalid MAC address format')
      expect(() => maskMACAddress('AA:BB:CC')).toThrow('Invalid MAC address format')
    })
  })

  describe('encryptWithPublicKey', () => {
    // Test RSA encryption with a mock public key
    // Note: In a real browser environment, this would use Web Crypto API
    // For unit tests, we'll test the error handling

    it('should throw error containing original reason for invalid PEM format', async () => {
      // 错误透传：message 应包含 "Failed to encrypt data" 前缀 + 原始原因
      await expect(encryptWithPublicKey('test', 'invalid-pem')).rejects.toThrow(
        /^Failed to encrypt data: /,
      )
    })

    // Note: Full encryption tests require a browser environment or polyfill
    // Integration tests should verify the full encryption flow
  })
})
