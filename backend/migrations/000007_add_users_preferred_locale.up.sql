-- users 表加 preferred_locale 字段（backend/i18n.md §5：异步邮件场景用此字段而非 Accept-Language）。
-- 可空，默认 NULL（首次请求 Accept-Language 采集后填充）。
ALTER TABLE users ADD COLUMN preferred_locale VARCHAR(10);
