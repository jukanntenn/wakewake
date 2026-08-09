/**
 * RSA encryption utilities
 * Uses Web Crypto API for frontend encryption
 */

// Node.js 兼容 polyfill（e2e.md §4.3）：
// 浏览器：window 存在 → 用 window.crypto（行为不变）。
// Node 22+：globalThis.crypto.subtle 可用（实测确认）。直接 import 本文件不会 ReferenceError。
// 生产前端零影响（浏览器走第一个分支）。
//
// 注意：类型依赖 DOM lib（Crypto / window）——前端 tsconfig 含 "dom" lib，E2E tsconfig 同步含。
const webcrypto: Crypto = typeof window !== 'undefined' ? window.crypto : globalThis.crypto

/**
 * Converts PEM format public key to ArrayBuffer
 */
function pemToArrayBuffer(pem: string): ArrayBuffer {
  // Remove PEM headers and whitespace
  const pemContents = pem
    .replace('-----BEGIN PUBLIC KEY-----', '')
    .replace('-----END PUBLIC KEY-----', '')
    .replace(/\s/g, '')

  // Base64 decode
  const binaryString = atob(pemContents)
  const bytes = new Uint8Array(binaryString.length)
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i)
  }
  return bytes.buffer
}

/**
 * Encrypts data using RSA public key
 * @param data Plaintext to encrypt (MAC address, Bemfa uid/secret_id/secret_key, etc.)
 * @param publicKeyPEM PEM format public key
 * @returns Base64 encoded ciphertext
 *
 * 注意：本函数只做加密，不修改数据。MAC 大写化由调用方 `formatMAC` 负责
 * （曾在此处 toUpperCase，导致 Bemfa uid 小写被转大写后 broker 拒绝连接）。
 */
export async function encryptWithPublicKey(data: string, publicKeyPEM: string): Promise<string> {
  try {
    // 1. Import public key
    const publicKeyBuffer = pemToArrayBuffer(publicKeyPEM)
    const publicKey = await webcrypto.subtle.importKey(
      'spki',
      publicKeyBuffer,
      {
        name: 'RSA-OAEP',
        hash: 'SHA-256',
      },
      false,
      ['encrypt'],
    )

    // 2. Encrypt data (原样加密，不做大小写转换)
    const encoder = new TextEncoder()
    const dataBuffer = encoder.encode(data)

    const encryptedBuffer = await webcrypto.subtle.encrypt(
      {
        name: 'RSA-OAEP',
      },
      publicKey,
      dataBuffer,
    )

    // 3. Return Base64 encoded ciphertext
    return arrayBufferToBase64(encryptedBuffer)
  } catch (error) {
    // 透传原始错误信息（不吞掉）——排障时能直接看到真实原因
    // （如非安全上下文下 "subtle is undefined"、PEM 格式损坏等）。
    const reason = error instanceof Error ? error.message : String(error)
    console.error('Encryption failed:', error)
    throw new Error(`Failed to encrypt data: ${reason}`)
  }
}

/**
 * Converts ArrayBuffer to Base64
 */
function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  return btoa(binary)
}

/**
 * Generates MAC address display format
 * Format: AA:**:**:**:**:FF (first and last segment visible)
 * @param mac MAC address
 */
export function maskMACAddress(mac: string): string {
  // Normalize format and convert to uppercase
  const normalized = mac.toUpperCase().replace(/[:-]/g, ':')

  // Validate format
  const parts = normalized.split(':')
  if (parts.length !== 6) {
    throw new Error('Invalid MAC address format')
  }

  // First and last segment visible, middle replaced with **
  return `${parts[0]}:**:**:**:**:${parts[5]}`
}

/**
 * Validates MAC address format
 */
export function isValidMAC(mac: string): boolean {
  const pattern = /^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$/
  return pattern.test(mac)
}

/**
 * Formats MAC address (unified to XX:XX:XX:XX:XX:XX format)
 */
export function formatMAC(mac: string): string {
  const cleaned = mac.toUpperCase().replace(/[-:]/g, '')
  return cleaned.replace(/(.{2})(?!$)/g, '$1:')
}
