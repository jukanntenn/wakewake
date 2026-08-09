/**
 * E2E crypto 工具（e2e.md §4.3 复用前端 crypto.ts + §4.4 RSA 密钥对生成）。
 *
 * 复用前端 frontend/src/lib/crypto.ts 的 encryptWithPublicKey（已加 webcrypto polyfill，
 * Node 22 可用）——保证加密行为与真实前端一致，避免重复实现漂移。
 *
 * RSA 密钥对生成（staticPublicKey fixture 用，等价 agent 真实生成的 2048-bit RSA）：
 * 用 node:crypto generateKeyPairSync。
 */
import { generateKeyPairSync } from 'node:crypto'

// 复用前端 crypto.ts（顶部已加 webcrypto polyfill，e2e.md §4.3）
// 路径相对 e2e/ 目录：../frontend/src/lib/crypto.ts
export {
  encryptWithPublicKey,
  maskMACAddress,
  isValidMAC,
  formatMAC,
} from '../../frontend/src/lib/crypto'

/** RSA 密钥对 PEM（SPKI public + PKCS8 private，e2e.md §4.4）。 */
export interface RSAKeyPairPEM {
  publicKey: string
  privateKey: string
}

/** 生成 2048-bit RSA 密钥对（PEM 格式，等价 agent 真实生成的）。 */
export function generateRSAKeyPairPEM(): RSAKeyPairPEM {
  const { publicKey, privateKey } = generateKeyPairSync('rsa', {
    modulusLength: 2048,
    publicKeyEncoding: { type: 'spki', format: 'pem' },
    privateKeyEncoding: { type: 'pkcs8', format: 'pem' },
  })
  return { publicKey, privateKey }
}

/** 生成随机 MAC 地址（EUI-48 格式，测试设备用）。 */
export function randomMac(): string {
  const bytes = Array.from({ length: 6 }, () => Math.floor(Math.random() * 256))
  // 第一字节第二位置 0（单播）
  bytes[0] &= 0xfe
  return bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join(':')
}
