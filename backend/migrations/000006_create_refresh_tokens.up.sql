-- refresh_tokens 表（database-design.md §表结构）
-- token_hash 存 SHA-256(明文 token) 的 hex（CHAR(64) = 64 hex = 256 bit），明文永不入库。
-- rotation：每次 refresh 把旧 token revoked_at 置值、签发新行。
-- 重放检测：已吊销 refresh 被复用 → 吊销该用户所有 refresh。
CREATE TABLE refresh_tokens (
    id          BIGSERIAL    PRIMARY KEY,
    user_id     BIGINT       NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  CHAR(64)     NOT NULL,
    expires_at  TIMESTAMPTZ  NOT NULL,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_refresh_tokens_hash    ON refresh_tokens (token_hash);
CREATE        INDEX idx_refresh_tokens_user    ON refresh_tokens (user_id);
CREATE        INDEX idx_refresh_tokens_expires ON refresh_tokens (expires_at);
