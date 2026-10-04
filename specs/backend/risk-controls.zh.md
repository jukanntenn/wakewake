# 风控控制（admin 运行时）

[English](risk-controls.md) | 中文

`/admin/*` 背后的运营侧滥用控制：每个杠杆的作用、不可移动的不变量、以及各状态的位置。决策记录见 [`../../.agents/rfcs/implemented/feature/2026-09-04-admin-risk-controls.zh.md`](../../.agents/rfcs/implemented/feature/2026-09-04-admin-risk-controls.zh.md)。

## 运行时句柄家族

所有运行时杠杆遵循同一模式，由维护模式首创：`AppState` 中 `Arc<RwLock>` 句柄后的内存真源，best-effort 持久化到 `data/` 下的 JSON 文件（启动恢复并覆盖配置默认值），每次 admin 变更写入 `admin_actions` 审计。抹掉 `data/` 卷会把控制项重置为配置默认 —— 生产 compose 挂载 `data/`，部署不丢。

| 句柄 | 文件 | Admin API |
|---|---|---|
| `MaintenanceHandle`（`service/maintenance.rs`） | `data/maintenance.json` | `GET/POST /admin/maintenance` |
| `MailerControl`（`service/mailer_control.rs`） | `data/mailer.json` | `GET/POST /admin/mailer` |
| `PowService` 难度旋钮（`service/pow.rs`） | `data/pow.json` | `GET/POST /admin/pow` |
| `IpBanStore`（`service/ip_ban.rs`） | `data/ip_bans.json` | `GET/POST/DELETE /admin/ip-bans` |

注册关闭即 maintenance 的 `registration_disabled` 档 —— 刻意不设独立开关（readonly/full 本就隐含关闭；「注册是否开放」只有一个事实来源）。受众规则：注册页轮询公开的 `/health/maintenance` 并禁用表单（任何启用的档位都拦截注册）；`MaintenanceBanner` 只在 `readonly` 档展示，因为其登录受众不受关注册影响。

## 邮件预算

三条发信路径按 UTC 日独立分账：`register`（注册时的验证邮件 —— 唯一收件人由攻击者任意指定的路径）、`resend`、`reset`。限额默认 500/200/300（`[mailer].max_*_emails_per_day`，0 = 不限），运行时可调。

不变量：

- **预算耗尽的降级与 `mailer.enabled = false` 完全一致**：注册静默照常（账号停留未验证，之后可 resend 补救）、resend 静默跳过（防枚举恒 200）、密码重置请求在路由预检（`would_send`）处 503。刻意不为耗尽引入专用错误码。
- **计数跨重启持久** —— 攻击者不能靠重启等出预算，运营者也不会误重置。
- 名额（`try_acquire`）在 SMTP 尝试**之前**消耗；SMTP 失败同样计入预算。不消耗计数的 `would_send` 预检供路由层判定与 admin 展示。
- 拒绝不静默：分路 `blocked` 计数、OTel `emails_blocked_total{path,reason}`、warn 日志。
- `verification_sent_at`（60s resend 冷却锚点）只在发送成功后写入 —— 失败与预算耗尽不烧冷却。

## PoW 难度

`0..=10`，默认 4（配置 `pow.difficulty`）。每个 challenge 内嵌签发时的难度并按自身值验证 —— 调旋钮不影响在途 challenge（10 分钟 TTL 是自然过渡窗口）。难度 0 完全关闭证明；视为调试便利而非生产状态。

## 未验证账号清理

每日 housekeeping（24h 周期，首 tick 在启动时）删除 `email_verified = false AND is_superuser = false AND created_at < now() - unverified_retention_days`（默认 7，0 = 关闭）的 `users`。FK 级联清掉 1:1 的 agent 行与 refresh token。

不变量：

- 账龄锚点是 **`created_at`，绝不用 `verification_sent_at`**：resend 端点防枚举恒 200，任何人都能每天刷新后者给抢占的地址续命。
- SQL 守卫（`email_verified = false`、`is_superuser = false`）是绝不触碰真实账号的双保险；两者都被集成测试钉死。
- 未验证用户不可能拥有 devices/integrations（登录被拦），此路径不可达 `devices.agent_id`/`integrations.agent_id` 的 `ON DELETE RESTRICT`。
- 删除按 1000 一批执行，洪水后的首次清理不是单条大事务。

## IP 封禁

条目为精确 IP（HashMap 直查）或 CIDR 前缀（对几十个条目线性扫描），可选 TTL；过期在读路径惰性判定，每日 housekeeping compact。匹配使用与限流相同的 client-IP 口径（`util::client_ip_from_headers`），其不可伪造性完全依赖 Caddy 对非 Cloudflare 直连的 403 —— 封禁层继承该拓扑契约。

不变量：

- 中间件挂在全局限流**之外**（banned IP 不占 governor 预算）；`/api/v1/health*` 豁免（探活不携带业务语义）。
- **Fail-open**：锁 poison 时中间件放行 —— 封禁层绝不能因自身故障打死全站。
- **自封守卫**拒绝任何会覆盖请求者自身 IP 的目标（422 `self_ban` 字段码）—— 否则运营者会被锁在唯一能解封的界面之外。
- 清理未验证账号的同一任务顺带 compact 过期封禁。

## 风控面板

`GET /admin/risk` 组合三类来源：`users`/`login_events` 的 DB 聚合（注册 24h/7d、未验证堆积 + 最老账龄、失败登录、含独立邮箱数的 top 失败 IP、top 被攻击邮箱）、运行时句柄快照（mailer 计数、PoW 难度、生效封禁数）、以及以 `rate_limited_since_start` + 运行时长暴露的进程内 429 计数。429 数字**重启归零并如实标注**；OTel 计数器与 CF 分析页是持久记录。admin 概览将其渲染为风控区块，每个 IP 旁的 **Ban** 按钮默认 24h TTL —— 处置快、误伤可自愈；确认长期恶意再在封禁列表升级为永久。
