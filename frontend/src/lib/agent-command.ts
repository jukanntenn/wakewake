// Agent 启动命令模板槽（specs/frontend/agent-onboarding.md / specs/backend/agent-distribution.md）。
// 两个生成器对应终端卡 Linux / Docker 双 tab；--server 由调用方传入当前站点 origin。
// 命令与 README 实际分发渠道一字不差，不虚构。

const INSTALL_SCRIPT_URL = 'https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh'
const DOCKER_IMAGE = 'jukanntenn/wakewake-agent:latest'

/** Linux 一行安装：install.sh 检测平台 → 下载 Release 资产 → sha256 校验 → 装 → 前台启动。 */
export function buildAgentInstallCommand(serverUrl: string, pairingCode: string): string {
  return `curl -fsSL ${INSTALL_SCRIPT_URL} | sh -s -- --server ${serverUrl} --code ${pairingCode}`
}

/** Docker 一行启动。--network host 是硬前提：受限广播（RFC 919）不出 bridge 网段。 */
export function buildAgentDockerCommand(serverUrl: string, pairingCode: string): string {
  return [
    'docker run -d',
    '--name wakewake-agent',
    '--network host',
    '--restart unless-stopped',
    `-e WAKEWAKE_SERVER_URL=${serverUrl}`,
    `-e WAKEWAKE_PAIRING_CODE=${pairingCode}`,
    '-v wakewake-agent-data:/data',
    DOCKER_IMAGE,
  ].join(' ')
}

/** Docker Compose YAML（推荐路径）：与 buildAgentDockerCommand 逐字段等价。
 *  环境值加引号，避免含冒号的 URL 触发 YAML plain scalar 歧义；
 *  末行以注释附带启动命令——复制粘贴即得合法 YAML 且自带说明。 */
export function buildAgentComposeYaml(serverUrl: string, pairingCode: string): string {
  return [
    '# compose.yaml',
    'services:',
    '  wakewake-agent:',
    `    image: ${DOCKER_IMAGE}`,
    '    container_name: wakewake-agent',
    '    network_mode: host # 硬前提：受限广播（RFC 919）不出 bridge 网段',
    '    restart: unless-stopped',
    '    environment:',
    `      WAKEWAKE_SERVER_URL: "${serverUrl}"`,
    `      WAKEWAKE_PAIRING_CODE: "${pairingCode}"`,
    '    volumes:',
    '      - wakewake-agent-data:/data',
    'volumes:',
    '  wakewake-agent-data:',
    '# 启动: docker compose up -d',
  ].join('\n')
}
