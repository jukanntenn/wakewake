-- device-sync-v3：三端同步状态机重构（specs/backend/device-sync-v3.md §7.1）。
-- 增量 ALTER（方案 A）：从当前 schema 演进到 §7.0 目标态。
-- 数据语义为冷启动重置（§7.1）：DROP 旧同步状态列 = 丢弃历史同步态；
-- 新观测列全 NULL，迁移后所有设备回到 cloud_status=not_observed，
-- 由 agent 首轮对账冷启动重建（§7.3 三态编码）。业务数据不受影响。

-- agents：投影单调版本（与数据写同事务自增，§5.4；ack 比对 + gap 重推依据）。
ALTER TABLE agents ADD COLUMN projection_version BIGINT NOT NULL DEFAULT 0;

-- devices：观测三态 + 漂移告警 + 单设备修复错误（替代旧的 sync_status/bemfa_status）。
ALTER TABLE devices ADD COLUMN bemfa_observed_name TEXT;
ALTER TABLE devices ADD COLUMN bemfa_observed_at   TIMESTAMPTZ;
ALTER TABLE devices ADD COLUMN last_drift_at       TIMESTAMPTZ;
ALTER TABLE devices ADD COLUMN last_drift_kind     VARCHAR(16);
ALTER TABLE devices ADD COLUMN last_error          TEXT;
ALTER TABLE devices DROP COLUMN sync_status;
ALTER TABLE devices DROP COLUMN bemfa_status;
ALTER TABLE devices DROP COLUMN bemfa_last_error;

-- integrations：MQTT 连接状态镜像 + 最近对账上报时间（替代 sync_status/last_synced_at）。
-- last_error 保留（承载 uid 解密失败等集成级错误）。
ALTER TABLE integrations ADD COLUMN mqtt_connected  BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE integrations ADD COLUMN last_report_at  TIMESTAMPTZ;
ALTER TABLE integrations DROP COLUMN sync_status;
ALTER TABLE integrations DROP COLUMN last_synced_at;
