-- 密码重置 per-email 节流（authentication.md §七层4，A-24）。
-- password_reset_sent_at：上次发送密码重置邮件的时间戳。
-- 用于 per-email 15min 冷却，防止同一邮箱被邮件轰炸（per-IP 3/hour 之外的第二道闸）。
ALTER TABLE users
    ADD COLUMN password_reset_sent_at TIMESTAMPTZ;
