-- ui-ux-risk-control §8.2：users 加 disabled_reason + disabled_by（当前态，enable 时清空）。
-- 当前态（users 表）vs 历史（admin_actions 表）的关系见 §0.7。
-- disabled_by ON DELETE SET NULL：即使 admin 账户被删，被禁用用户的记录仍在。
ALTER TABLE users ADD COLUMN disabled_reason TEXT;
ALTER TABLE users ADD COLUMN disabled_by   BIGINT REFERENCES users(id) ON DELETE SET NULL;
