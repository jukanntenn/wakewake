/**
 * PostgreSQL 直查（e2e.md §4.5）。
 *
 * 通过 docker exec psql 执行 SQL（不依赖 Node pg 驱动，减少依赖）。
 * 容器名 wakewake-e2e-postgres-1（compose 默认命名）。
 *
 * 用途：四重证据之一——验证副作用（sync_status、wake 记录、is_active 等）。
 */
import { execSync } from 'node:child_process'

// compose 项目名取目录名（e2e/）→ 容器名 e2e-postgres-1。
// 规范 §4.5 写 wakewake-e2e-postgres-1 假设 compose project=wakewake-e2e，实际是 e2e。
const PG_CONTAINER = process.env.PG_CONTAINER ?? 'e2e-postgres-1'
const PG_USER = process.env.POSTGRES_USER ?? 'wakewake'
const PG_DB = process.env.POSTGRES_DB ?? 'wakewake_test'

/** 执行 SQL，返回 stdout（trim）。通过 stdin 传 SQL（支持多行，如 PEM 公钥）。 */
export function execSql(sql: string): string {
  // 用 stdin 传 SQL（-c 不支持多行内容，如 PEM 公钥含换行）
  return execSync(
    `docker exec -i ${PG_CONTAINER} psql -U ${PG_USER} -d ${PG_DB} -t -A`,
    { input: sql },
  )
    .toString()
    .trim()
}

/**
 * 查 device 的观测三态 + 漂移（device-sync-v3 §9.3）。
 * 返回 {observed_name, observed_at, drift_kind, last_error}，调用方据此派生 cloud_status。
 */
export interface DeviceObservation {
  observed_name: string | null
  observed_at: string | null
  drift_kind: string | null
  last_error: string | null
}
export function getDeviceObservation(did: string): DeviceObservation {
  const raw = execSql(
    `SELECT bemfa_observed_name, bemfa_observed_at, last_drift_kind, last_error FROM devices WHERE did = '${did}';`,
  )
  // psql 默认 | 分隔列，NULL 显示为空
  const [observed_name, observed_at, drift_kind, last_error] = raw.split('|').map((s) => {
    const t = s.trim()
    return t === '' || t === '(null)' ? null : t
  })
  return {
    observed_name,
    observed_at,
    drift_kind,
    last_error,
  }
}

/**
 * 查 agent.projection_version（device-sync-v3 §5.4 单调版本）。
 */
export function getProjectionVersion(agentId: number): number {
  const raw = execSql(`SELECT projection_version FROM agents WHERE id = ${agentId};`)
  return parseInt(raw, 10) || 0
}

/** 查 device 的 wake 记录数（唤醒副作用）。 */
export function getWakeCount(deviceId: string): number {
  const raw = execSql(`SELECT COUNT(*) FROM wakes WHERE device_did = '${deviceId}';`)
  return parseInt(raw, 10) || 0
}

/** 查 wake 记录的 status（success/failed/expired）。 */
export function getWakeStatus(deviceId: string): string {
  return execSql(
    `SELECT status FROM wakes WHERE device_did = '${deviceId}' ORDER BY created_at DESC LIMIT 1;`,
  )
}

/** 查 integration 的 secret 字段是否为 RSA 密文（非明文，e2e.md §7.2 场景1）。 */
export function getIntegrationConfigField(
  provider: string,
  field: string,
): string {
  // config 是 jsonb，->> 取文本字段
  return execSql(
    `SELECT config->>'${field}' FROM integrations WHERE provider = '${provider}';`,
  )
}

/** 管理员直改 is_active（e2e.md §5.5 adminUser fixture + §8 测试）。 */
export function setUserActive(userId: number, active: boolean): void {
  const disabledAt = active ? 'null' : 'now()'
  execSql(
    `UPDATE users SET is_active = ${active}, disabled_at = ${disabledAt} WHERE id = ${userId};`,
  )
}

/** 管理员直改 is_superuser（e2e.md §5.5 adminUser，API 无法注册 superuser）。 */
export function setUserSuperuser(userId: number, superuser: boolean): void {
  execSql(`UPDATE users SET is_superuser = ${superuser} WHERE id = ${userId};`)
}

/** 直改 agent.public_key（e2e.md §7.2 staticPublicKey fixture，场景1 不启 agent）。 */
export function setAgentPublicKey(userId: number, publicKeyPem: string): void {
  // 转义单引号（PEM 含换行，用 dollar-quoting 避免）
  execSql(
    `UPDATE agents SET public_key = $PEM$${publicKeyPem}$PEM$ WHERE user_id = ${userId};`,
  )
}
