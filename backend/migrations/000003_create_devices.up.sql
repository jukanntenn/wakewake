-- devices 表（database-design.md §表结构）
-- mac_encrypted base64 密文全链路统一命名（DB + API + SSE）。
-- agent_id FK + ON DELETE RESTRICT（有设备的 agent 不许删）。
CREATE TABLE devices (
    id            BIGSERIAL    PRIMARY KEY,
    did           UUID         NOT NULL,
    user_id       BIGINT       NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    agent_id      BIGINT       NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
    name          VARCHAR(100) NOT NULL,
    mac_encrypted TEXT         NOT NULL,
    mac_display   VARCHAR(17)  NOT NULL,
    description   VARCHAR(255),
    sync_status   VARCHAR(20)  NOT NULL DEFAULT 'syncing',
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_devices_did         ON devices (did);
CREATE        INDEX idx_devices_user_id     ON devices (user_id);
CREATE        INDEX idx_devices_agent_id    ON devices (agent_id);
