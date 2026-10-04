import { describe, it, expect } from 'vitest'
import { resolveAddDeviceGate } from './add-device-guard'

// 前置状态机（specs/frontend/agent-onboarding.md）：顺序固定，命中即返回。

describe('resolveAddDeviceGate', () => {
  it('gates on unpaired agent first, regardless of quota or context', () => {
    expect(
      resolveAddDeviceGate({
        hasAgentPublicKey: false,
        maxDevices: 2,
        deviceCount: 2,
        isSecureContext: false,
      }),
    ).toBe('agent')
  })

  it('gates on quota when paired and full', () => {
    expect(
      resolveAddDeviceGate({
        hasAgentPublicKey: true,
        maxDevices: 2,
        deviceCount: 2,
        isSecureContext: true,
      }),
    ).toBe('quota')
  })

  it('skips quota check when limits are unknown (stale session) — server backstop remains', () => {
    expect(
      resolveAddDeviceGate({
        hasAgentPublicKey: true,
        maxDevices: undefined,
        deviceCount: 5,
        isSecureContext: true,
      }),
    ).toBeNull()
  })

  it('gates on insecure context after agent and quota pass', () => {
    expect(
      resolveAddDeviceGate({
        hasAgentPublicKey: true,
        maxDevices: 2,
        deviceCount: 0,
        isSecureContext: false,
      }),
    ).toBe('insecure')
  })

  it('allows offline-but-paired agents (public key present is the real precondition)', () => {
    expect(
      resolveAddDeviceGate({
        hasAgentPublicKey: true,
        maxDevices: 2,
        deviceCount: 0,
        isSecureContext: true,
      }),
    ).toBeNull()
  })
})
