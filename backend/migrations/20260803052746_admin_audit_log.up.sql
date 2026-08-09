-- admin_actions 审计表（admin 管理台风控审计）。
-- 每个 admin 写操作记录一行：谁（actor_id）在何时（created_at）对什么 target 做了什么 action。
-- action 字符串约定："<resource>.<verb>"，如 "user.disable" / "device.resync" / "agent.disconnect"。
-- target 三列均为可空：不同 action 命中不同的 target（disable→target_user_id，resync→target_device_did 等）。
-- detail JSONB 存操作附加上下文（如重置密码不含明文、仅记 "forced": true）。
CREATE TABLE admin_actions (
    id                 BIGSERIAL    PRIMARY KEY,
    actor_id           BIGINT       NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action             VARCHAR(50)  NOT NULL,
    target_user_id     BIGINT,
    target_agent_id    BIGINT,
    target_device_did  UUID,
    detail             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE INDEX idx_admin_actions_actor_time   ON admin_actions (actor_id, created_at DESC);
CREATE INDEX idx_admin_actions_target_user  ON admin_actions (target_user_id, created_at DESC);
CREATE INDEX idx_admin_actions_action_time  ON admin_actions (action, created_at DESC);
