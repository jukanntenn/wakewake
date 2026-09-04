import { describe, it, expect } from 'vitest'
import { buildAgentInstallCommand, buildAgentDockerCommand } from './agent-command'

describe('buildAgentInstallCommand', () => {
  it('is a single line embedding server origin and pairing code', () => {
    const cmd = buildAgentInstallCommand('https://wakewake.example.com', 'a1b2c3d4e5f60718')
    expect(cmd).not.toContain('\n')
    expect(cmd).toBe(
      'curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh | sh -s -- --server https://wakewake.example.com --code a1b2c3d4e5f60718',
    )
  })
})

describe('buildAgentDockerCommand', () => {
  it('requires host networking and persistence, embeds env config', () => {
    const cmd = buildAgentDockerCommand('https://wakewake.example.com', 'a1b2c3d4e5f60718')
    expect(cmd).not.toContain('\n')
    expect(cmd).toContain('--network host')
    expect(cmd).toContain('--restart unless-stopped')
    expect(cmd).toContain('-e WAKEWAKE_SERVER_URL=https://wakewake.example.com')
    expect(cmd).toContain('-e WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718')
    expect(cmd).toContain('-v wakewake-agent-data:/data')
    expect(cmd).toContain('jukanntenn/wakewake-agent:latest')
  })
})
