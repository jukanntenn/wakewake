-- integrations 增加错误反馈列：
--   last_error      —— 最近一次同步失败原因（agent 上报 IntegrationReady success:false 时写入）
--   last_synced_at  —— 最近一次成功同步时间（success:true 时更新）
-- 两列均可空，存量行默认 NULL（未同步/无错误）。
ALTER TABLE integrations ADD COLUMN last_error TEXT;
ALTER TABLE integrations ADD COLUMN last_synced_at TIMESTAMPTZ;
