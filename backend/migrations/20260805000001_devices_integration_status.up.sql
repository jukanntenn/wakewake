-- devices 增加 per-device 的巴法云同步状态列（state-as-truth 重构）：
--   bemfa_status     —— 该设备在巴法云集成的同步结果（synced/error/permanent_error），
--                       由 agent 上报 IntegrationStatus.devices[].status 写入。
--                       可空：存量行或无集成时为 NULL。
--   bemfa_last_error —— 最近一次同步失败原因（status 为 error/permanent_error 时填充）。
-- 两列均可空，存量行默认 NULL（未对账/无集成）。
ALTER TABLE devices ADD COLUMN bemfa_status VARCHAR(20);
ALTER TABLE devices ADD COLUMN bemfa_last_error TEXT;
