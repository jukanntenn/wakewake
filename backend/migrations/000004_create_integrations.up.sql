-- integrations 表（database-design.md §表结构）
-- config JSONB 混合存储：非敏感字段明文，敏感字段（provider schema 标 secret:true）存 base64 RSA 密文。
-- 以 (user_id, provider) 寻址，无外部 UUID（URL 用 provider，user_id 从 JWT 取）。
CREATE TABLE integrations (
    id            BIGSERIAL    PRIMARY KEY,
    user_id       BIGINT       NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    agent_id      BIGINT       NOT NULL REFERENCES agents(id) ON DELETE RESTRICT,
    provider      VARCHAR(50)  NOT NULL,
    config        JSONB        NOT NULL DEFAULT '{}',
    sync_status   VARCHAR(20)  NOT NULL DEFAULT 'syncing',
    enabled       BOOLEAN      NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_integrations_user_provider ON integrations (user_id, provider);
CREATE        INDEX idx_integrations_agent_id      ON integrations (agent_id);
