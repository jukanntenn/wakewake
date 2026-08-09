-- 邮箱验证：注册后发验证邮件，点击链接确认。
-- email_verified：验证状态（存量用户视为已验证 TRUE，DEFAULT TRUE 保护）。
-- verification_sent_at：上次发验证邮件时间（resend 限流，防邮件轰炸）。
ALTER TABLE users
    ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN verification_sent_at TIMESTAMPTZ;
