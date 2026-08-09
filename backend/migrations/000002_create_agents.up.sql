-- agents 表（database-design.md §表结构）
-- pairing_code 单凭证（16 hex）；public_key PEM nullable until first SSE connect。
-- 高频更新 last_seen → autovacuum 调优（perf-est.md §10.3）。
CREATE TABLE agents (
    id            BIGSERIAL    PRIMARY KEY,
    user_id       BIGINT       NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    aid           UUID         NOT NULL,
    name          VARCHAR(100) NOT NULL,
    pairing_code  CHAR(16)     NOT NULL,
    public_key    TEXT,
    last_seen     TIMESTAMPTZ,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_agents_aid          ON agents (aid);
CREATE UNIQUE INDEX idx_agents_pairing_code ON agents (pairing_code);
CREATE        INDEX idx_agents_user_id      ON agents (user_id);

-- agents.last_seen 高频更新，降低 autovacuum 阈值防 bloat（perf-est.md §10.3）。
ALTER TABLE agents SET (autovacuum_vacuum_scale_factor = 0.05);
