# WRFC: resend 限流分组与诚实的冷却 UX

Status: implemented

[English](2026-09-05-resend-rate-limit-and-cooldown-ux.md) | 中文

## Problem

验证邮件 resend 端点(`POST /auth/verify-email/resend`)曾挂在 password-reset governor 组:per-IP 3 次/小时(GCRA 突发 3 个,每 20 分钟回 1 个令牌)。同一个按钮被三层口径不一致的保护管着:

1. 服务层:per-user 60 s 冷却,冷却内静默恒 200(防枚举)——自己从不发 429。
2. 中间件:per-IP 3/小时(429 的唯一可能来源)。
3. 前端:任何 429 都显示固定的"请等待一分钟"toast;`Retry-After` 从未被解析。

邮件迟到的真实用户大约每分钟点一次。每次点击都烧掉一个 IP 令牌——即使服务层静默 no-op——所以三次点击就耗尽桶,之后最长 20 分钟内每次点击都收到 429,而 toast 声称只需一分钟。用户等了"远超一分钟"再点,得到同一句话:即线上报告的症状。前端还毫无倒计时,变相鼓励了耗尽桶的连点。

## Decision

- **resend 移入独立 governor 组:per-IP 6/小时**(`email_resend_rate_limit_layer`,`middleware/rate_limit.rs`),与 password-reset 的 3/小时解耦。防邮件轰炸仍为三层:per-user 60 s 冷却、per-user 200 封 resend/天预算(`[mailer].max_resend_emails_per_day`)、per-IP 6/小时。
- **check-email 页用实时倒计时禁用 resend 按钮**:挂载即 60 s(注册后挂载时刻 ≈ 发信时刻,与服务层冷却对齐),每次发送后重置 60 s;429 时倒计时取 `max(60, Retry-After)`。
- **`ApiError` 解析 `Retry-After`**(RFC 9110 §10.2.3 的 delta-seconds 形态;governor 恒发该形态)为 `retryAfterSeconds`,429 toast 如实陈述真实等待,不再是固定的"一分钟"。
- 服务层恒 200 的防枚举语义不变;IP 级 429 在 handler 之前触发,不泄露账号存在性。

## Alternatives considered

- **保持 3/小时,只修文案** — 否决:诚实的文案让锁定可见,但仍然用最长 20 分钟的无声等待惩罚合法的着急用户;per-user 两层已约束单账号轰炸,IP 层唯一不可替代的职责是跨账号枚举,6/小时同样能界住。
- **resend 完全不做 IP 级限流** — 否决:它是"注册一批受害地址账号(register 本身有 PoW + 10/min/IP)再从同一 IP 触发 resend"的唯一跨账号约束。
- **把服务层冷却显式以 429 返回** — 否决:会破坏防枚举契约(与账号状态绑定的 429 会区分"存在的未验证账号"与"未知邮箱")。

## Consequences

- NAT 后的家庭或办公室共享 6/小时桶;一个 IP 后所有用户一小时内合计六次 resend 之后需等待令牌回补。倒计时与真实的 `Retry-After` 文案让这个状态可见而非玄学。
- 倒计时常量(60 s)在前端(`check-email/page.tsx`)与后端(`RESEND_COOLDOWN_SECS`)各有一份,靠注释耦合而非配置——对一个从不参与服务端 gating 的展示近似值,可接受。
