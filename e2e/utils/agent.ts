/**
 * Agent 容器生命周期管理（e2e.md §6.4）。
 *
 * agent 容器启动需 WAKEWAKE_PAIRING_CODE，但 pairing_code 是测试运行时从 server 取得的。
 * 方案：docker-compose.e2e.yml 的 agent 服务加 profiles:[agent]，默认不启动。
 * pairedAgent fixture 动态 docker compose --profile agent up 并注入运行时 pairing_code。
 *
 * WoL 验证：agent 容器内置 UDP sniffer（s6 管 wol-sniffer 进程），捕获 magic packet
 * 写 /tmp/wol_packets.log（e2e.md §6.3）。getWolPackets() 读该文件解析 JSON。
 */
import { execSync } from "node:child_process";
import { getDefaultAgent } from "./api";
import type { KyInstance } from "ky";
import { waitFor } from "./shared";

const COMPOSE_FILE = "docker-compose.e2e.yml";
const AGENT_CONTAINER = "e2e-agent-1";

/** WoL 包记录（wol-sniffer 写 /tmp/wol_packets.log 的 JSON 行）。 */
export interface WolPacket {
  received_at: string;
  mac: string;
  raw_hex: string;
}

/** Agent 容器管理器（e2e.md §6.4 AgentContainer）。 */
export class AgentContainer {
  constructor(
    private readonly name: string,
    private readonly pairingCode: string,
  ) {}

  /** 启动 agent 容器（注入运行时 pairing_code）。可选 browserClient 轮询 status=online。 */
  async start(browserClient?: KyInstance): Promise<void> {
    // .env 方式注入 pairing_code（compose 读 shell env）
    execSync(
      `WAKEWAKE_PAIRING_CODE=${this.pairingCode} ` +
        `docker compose -f ${COMPOSE_FILE} --profile agent up -d agent`,
      { stdio: "inherit" },
    );
    if (browserClient) {
      await this.waitForOnline(browserClient);
    } else {
      await this.waitForOnline();
    }
  }

  /** 停止 agent 容器（保留 volume，密钥不丢）。 */
  async stop(): Promise<void> {
    execSync(`docker compose -f ${COMPOSE_FILE} stop agent`, {
      stdio: "inherit",
    });
  }

  /** 重启 agent 容器（重连测试用，e2e.md §7.3）。 */
  async restart(): Promise<void> {
    execSync(`docker compose -f ${COMPOSE_FILE} restart agent`, {
      stdio: "inherit",
    });
    await this.waitForOnline();
  }

  /** 轮询 GET /agents/default 直到 status=online（agent 已连 SSE + 上报公钥）。
   *  不传 client 时仅等容器启动（restart 后 sniffer 重新读 log）。 */
  async waitForOnline(browserClient?: KyInstance): Promise<void> {
    if (!browserClient) {
      await new Promise((resolve) => setTimeout(resolve, 5000));
      return;
    }
    await waitFor(
      async () => getDefaultAgent(browserClient),
      (agent) => agent.status === "online",
      { timeoutMs: 60_000, intervalMs: 2000 },
    );
  }

  /** 读 WoL 包记录（tcpdump 捕获，e2e.md §6.3 双层证据①）。
   *  sniffer 用 tcpdump -l 持续追加，每个 UDP:9 包触发一行时间戳 + 若干 hex 行。
   *  解析：统计含 "localhost.9: UDP" 或 "127.0.0.1.9" 的包行数。 */
  async getWolPackets(): Promise<WolPacket[]> {
    try {
      const raw = execSync(
        `docker exec ${this.name} cat /tmp/wol_packets.log 2>/dev/null || true`,
      )
        .toString()
        .trim();
      if (!raw) return [];
      // tcpdump 输出：每包一行含 "IP ... > ...: UDP, length N"
      const packetLines = raw
        .split("\n")
        .filter((l) => l.includes(": UDP,") && l.includes(".9:"));
      return packetLines.map((line, i) => ({
        received_at: line.split(" ")[0] ?? new Date().toISOString(),
        mac: "",
        raw_hex: line,
      }));
    } catch {
      return [];
    }
  }

  /** 清空 WoL 包记录（测试间隔离）。 */
  async clearWolPackets(): Promise<void> {
    try {
      execSync(
        `docker exec ${this.name} sh -c 'echo -n > /tmp/wol_packets.log'`,
      );
    } catch {
      // 容器可能未起，忽略
    }
  }
}

export { AGENT_CONTAINER };
