import { describe, it, expect } from 'vitest'
import {
  buildAgentInstallCommand,
  buildAgentDockerCommand,
  buildAgentComposeYaml,
} from './agent-command'

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

describe('buildAgentComposeYaml', () => {
  const yaml = buildAgentComposeYaml('https://wakewake.example.com', 'a1b2c3d4e5f60718')

  it('mirrors the docker run command field by field', () => {
    expect(yaml).toContain('image: jukanntenn/wakewake-agent:latest')
    expect(yaml).toContain('container_name: wakewake-agent')
    expect(yaml).toContain('network_mode: host')
    expect(yaml).toContain('restart: unless-stopped')
    expect(yaml).toContain('WAKEWAKE_SERVER_URL: "https://wakewake.example.com"')
    expect(yaml).toContain('WAKEWAKE_PAIRING_CODE: "a1b2c3d4e5f60718"')
    expect(yaml).toContain('- wakewake-agent-data:/data')
  })

  it('declares the top-level named volume and carries the up command as a trailing comment', () => {
    const lines = yaml.split('\n')
    // 顶层 volumes 声明(无缩进)与 service 卷挂载(两空格缩进 + 列表)同时在场。
    expect(lines).toContain('volumes:')
    expect(lines).toContain('  wakewake-agent-data:')
    expect(lines).toContain('      - wakewake-agent-data:/data')
    expect(lines.at(-1)).toBe('# 启动: docker compose up -d')
    // service 键与顶层 volumes 键都以无缩进/两缩进出现,YAML 结构即由这些行构成。
    expect(lines).toContain('services:')
    expect(lines).toContain('  wakewake-agent:')
  })
})
