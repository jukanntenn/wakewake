/**
 * Docker 容器工具（e2e.md §3.2 utils/docker.ts）。
 * 容器 logs/exec，供调试与 agent 容器管理用。
 */
import { execSync } from 'node:child_process'

const COMPOSE_FILE = 'docker-compose.e2e.yml'

/** docker compose 命令前缀（在 e2e/ 目录下运行）。 */
function composeCmd(args: string): string {
  return `docker compose -f ${COMPOSE_FILE} ${args}`
}

/** 取容器日志（失败时调试，e2e.md §10.1）。 */
export function getServiceLogs(service: string, lines = 200): string {
  return execSync(
    composeCmd(`logs --tail=${lines} ${service}`),
  ).toString()
}

/** docker exec 在容器内跑命令。 */
export function dockerExec(
  container: string,
  cmd: string,
  opts: { silent?: boolean } = {},
): string {
  const result = execSync(`docker exec ${container} sh -c "${cmd.replace(/"/g, '\\"')}"`).toString()
  if (!opts.silent) {
    process.stdout.write(result)
  }
  return result.trim()
}

/** 检查容器运行状态。 */
export function isContainerRunning(container: string): boolean {
  try {
    const state = execSync(`docker inspect -f '{{.State.Running}}' ${container}`).toString().trim()
    return state === 'true'
  } catch {
    return false
  }
}

/** 取容器退出状态码（agent 401 终止验证，e2e.md §7.3）。 */
export function getContainerExitCode(container: string): number | null {
  try {
    const code = execSync(`docker inspect -f '{{.State.ExitCode}}' ${container}`).toString().trim()
    return parseInt(code, 10) || null
  } catch {
    return null
  }
}
