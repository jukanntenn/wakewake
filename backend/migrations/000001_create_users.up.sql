-- users 表（database-design.md §表结构）
-- email 唯一 + 作登录名；password bcrypt $2b$ cost=10（crate 固定 60 字节）。
CREATE TABLE users (
    id            BIGSERIAL    PRIMARY KEY,
    email         VARCHAR(255) NOT NULL UNIQUE,
    password      VARCHAR(60)  NOT NULL,
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    disabled_at   TIMESTAMPTZ,
    is_superuser  BOOLEAN      NOT NULL DEFAULT FALSE,
    last_login    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_users_email ON users (email);
