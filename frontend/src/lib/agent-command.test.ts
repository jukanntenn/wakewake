import { describe, it, expect } from 'vitest'
import { buildAgentLaunchCommand } from './agent-command'

describe('buildAgentLaunchCommand', () => {
  it('embeds server origin and pairing code in the final command', () => {
    const cmd = buildAgentLaunchCommand('https://wakewake.example.com', 'a1b2c3d4e5f60718')
    const lines = cmd.split('\n')
    expect(lines).toHaveLength(4)
    expect(lines[3]).toBe(
      './target/release/wakewake-agent --server https://wakewake.example.com --pairing-code a1b2c3d4e5f60718',
    )
  })

  it('starts with the README-identical build steps', () => {
    const cmd = buildAgentLaunchCommand('https://x.example', 'ffff')
    expect(cmd).toContain('git clone https://github.com/jukanntenn/wakewake.git')
    expect(cmd).toContain('cargo build --release -p wakewake-agent')
  })
})
