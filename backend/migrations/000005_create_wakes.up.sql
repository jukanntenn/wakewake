-- wakes 审计表（database-design.md §表结构）
-- user_id FK + CASCADE；device/agent 不设 FK，保留 device_did/device_name 冗余快照。
-- 写最多的表；按月分区或定时清理（90 天）。写入异步（buffered channel）。
CREATE TABLE wakes (
    id           BIGSERIAL    PRIMARY KEY,
    user_id      BIGINT       NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_did   UUID         NOT NULL,
    device_name  VARCHAR(100) NOT NULL,
    type         VARCHAR(20)  NOT NULL,
    status       VARCHAR(20)  NOT NULL,
    message      VARCHAR(255),
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_wakes_user_time   ON wakes (user_id, created_at DESC);
CREATE INDEX idx_wakes_created_at  ON wakes (created_at);
