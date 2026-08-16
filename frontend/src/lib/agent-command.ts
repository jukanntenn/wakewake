// Agent 启动命令模板槽（specs/frontend/agent-onboarding.md 唯一来源）。
// --server 由调用方传入当前站点 origin；一键启动机制落地时只改 buildAgentLaunchCommand。

export function buildAgentLaunchCommand(serverUrl: string, pairingCode: string): string {
  // 命令与 README.md Quick start 保持一致；不虚构未发布的分发渠道（docker / 下载链接）。
  return [
    'git clone https://github.com/jukanntenn/wakewake.git',
    'cd wakewake/backend',
    'cargo build --release -p wakewake-agent',
    `./target/release/wakewake-agent --server ${serverUrl} --pairing-code ${pairingCode}`,
  ].join('\n')
}
