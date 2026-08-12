-- ui-ux-risk-control §8.3：login_events 表（登录审计，30 天保留）。
-- user_id 可空（撞库时 email 可能对应无账户，仍要记录）。
-- email 冗余（user_id 为空时仍需知道试了哪个 email），NOT NULL。
-- ip_address 用 PostgreSQL 原生 INET（支持子网查询）。
-- failure_code：INVALID_CREDENTIALS / USER_DISABLED / EMAIL_NOT_VERIFIED / RATE_LIMITED；成功时 NULL。
-- 三个索引支撑：按用户查登录历史、按 email 查撞库、按 IP 查可疑来源。
CREATE TABLE login_events (
    id           BIGSERIAL    PRIMARY KEY,
    user_id      BIGINT       REFERENCES users(id) ON DELETE CASCADE,
    email        VARCHAR(255) NOT NULL,
    success      BOOLEAN      NOT NULL,
    ip_address   INET,
    user_agent   TEXT,
    failure_code VARCHAR(50),
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_login_events_user_time  ON login_events (user_id, created_at DESC);
CREATE INDEX idx_login_events_email_time ON login_events (email, created_at DESC);
CREATE INDEX idx_login_events_ip_time    ON login_events (ip_address, created_at DESC);
