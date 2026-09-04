# WRFC: Admin 运行时风控控制

Status: implemented

[English](2026-09-04-admin-risk-controls.md) | 中文

## Problem

准备面向全球公众运营暴露了运营侧的缺口：产品有厚实的逐请求防御（PoW、每 IP governor 限流、登录锁定、防枚举），但对真正会烧穿 2 GB VPS 的滥用场景缺少带内处置手段：

- **邮件支出无上界且运行时不可控。** 注册是唯一「收件人地址由攻击者任意指定」的发信路径；分布式 IP 绕过每 IP 限流，PoW 难度 4 对 bot 是毫秒级成本。除了 SMTP 配额，向垃圾邮件陷阱发信会摧毁发件信誉甚至导致 SMTP 账号被封 —— 真实用户的密码重置随之死亡，构成全认证面 DoS。`mailer.enabled` 启动期冻结，攻击进行中唯一的应对是重启进程。
- **未验证账号永久堆积。** 每次注册写入一行 `users` 加一行 1:1 的 `agents`，无任何清理。注册洪水永久污染数据库；抢占者用受害者邮箱注册但不验证，受害者之后注册将无限期吃到 409 `USER_EXISTS`。
- **应用层无 IP 封禁。** 唯一封禁途径是 Cloudflare 控制台 —— 带外操作，且 CF 免费版 5 条 WAF 规则太稀缺，不该花在单个 IP 上。
- **滥用不可见。** 注册速率、发信消耗、失败登录聚合、429 计数都没有 admin 面；发现依赖 CF 告警或翻日志。
- 注册开关本身已存在（maintenance `registration_disabled`），但 UX 有漏：注册页让用户先算完 PoW 才吃到 403 toast；dashboard 全局告警横幅投错受众（登录用户不受关注册影响）。

## Decision

运行时控制沿用 `MaintenanceHandle` 模式 —— `Arc<RwLock>` 句柄下的内存真源，持久化到 `data/` 下的 JSON 文件，启动恢复，每次 admin 变更写入 `admin_actions` 审计。落地四个句柄/控制：

1. **注册开关留在 maintenance**（`registration_disabled` 档；单一事实来源 —— `readonly`/`full` 本就隐含关注册）。打磨：注册页轮询公开的 `/health/maintenance`，关闭期间禁用表单并显示公告，不再让用户白算一次 PoW；`MaintenanceBanner` 只在 `readonly` 档展示（注册关闭不影响其登录受众）。
2. **`MailerControl`**（`service/mailer_control.rs`，`data/mailer.json`）：运行时总闸 + 分路 UTC 日预算，三路分账 —— `register`/`resend`/`reset`（默认 500/200/300，0 = 不限）。分路预算是根因修复：注册洪水烧不干密码重置的份额。预算耗尽时各路径降级为与 `mailer.enabled=false` 完全相同的行为（注册静默跳过 / resend 静默 / 重置请求 503）—— 零新增错误码、零新增用户端 UX。计数持久化，重启不重开闸门。`MailerService` 发送前先占名额（`try_acquire`）；`would_send` 是不消耗计数的只读预检，供重置路由的 503 判定。
3. **PoW 难度是运行时旋钮**（`service/pow.rs`，`data/pow.json`，`GET/POST /admin/pow`，0..=10）：从 4 拉到 6–7 使 bot 成本提升 256–4096 倍 —— 「开放」与「关闭」之间的中间档。每个 challenge 内嵌签发时的难度，旋钮调整不影响在途 challenge。
4. **`IpBanStore`**（`service/ip_ban.rs`，`data/ip_bans.json`）：精确 IP（HashMap 直查）与 CIDR（线性扫描 —— 条目量级在几十）两种条目，TTL 或永久，过期在读路径惰性判定，每日 housekeeping 任务 compact。中间件（`middleware/ip_ban.rs`）挂全局限流之外（banned IP 不占 governor 预算），health 豁免，fail-open —— 封禁层自身故障不能打死全站。IP 可信度依托既有拓扑：Caddy 对非 Cloudflare 直连 403，与限流共用的 client-IP 口径不可伪造。自封守卫拒绝任何覆盖请求者自身 IP 的目标（422），杜绝把自己锁在门外的单向事故。
5. **未验证清理**（`user_repo::purge_unverified`，`security.unverified_retention_days`，默认 7，0 = 关闭）：每日批量删除 `email_verified = false AND is_superuser = false AND created_at < now() - N 天`；FK 级联清 `agents`/`refresh_tokens`。锚点用 `created_at` 而非 `verification_sent_at` —— resend 端点防枚举恒 200，任何第三方都能每天刷新该时间戳给抢占账号「续命」。分批（1000/批）避免洪水后首清的单条大事务。
6. **`GET /admin/risk`** 聚合 DB 信号（注册 24h/7d、未验证堆积 + 账龄、失败登录、来自 `login_events` 的 top 失败 IP / top 被攻击邮箱）、运行时句柄快照（mailer、PoW、封禁数）、进程内 429 计数（`metrics::rate_limited_snapshot` 可读；重启归零 —— 面板如实标注，CF 分析页仍是对账来源）。admin 概览页渲染为「风控」区块，top 失败 IP 旁一键 **Ban**（默认 24h TTL，处置快、误伤可自愈）。新增 OTel 计数器：`emails_total{path}`、`emails_blocked_total{path,reason}`、`unverified_purged_total`、`http_rate_limited_total`。

顺带修复两个相邻缺陷：验证信重发只在发送**成功**后记 `verification_sent_at`（对齐 reset 的刻意语义 —— 失败不再烧 60s 冷却）；mailer 503 预检从 `is_enabled()` 改为 `would_send(Reset)`，让运行时闸门参与判定。

## Alternatives considered

**独立的一等注册开关（独立 API/持久化）。** 否决：`readonly`/`full` 本就隐含关注册，独立开关没有功能增量，反而制造「注册是否开放」的第二事实来源；维护页的档位文案本来就是「Pause registration」而非维护措辞。

**仅总闸（无预算）。** 否决为不完整：单开关下，为挡注册洪水而关邮件也会让真实用户的密码重置 503 —— 攻击者借运营者之手达成认证 DoS。分路预算正是解除该耦合的机制。

**全局邮件预算而非分路。** 否决：单一池子会让注册垃圾耗干重置份额 —— 同样的 DoS 换了个入口。

**计数入库。** 否决：封禁与预算是运维运行态而非业务数据；文件句柄免去 migration，且沿用 `maintenance.json` 先例。

**DB 封禁表。** 同因否决；热路径内存驻留，文件仅作持久化。

**admin 手动「立即清理」按钮。** 否决：定时任务是根因解法；洪水期间 24 小时之差无关紧要，按钮只增加攻击面 API。

**阈值告警 / 自动通知。** 缓议：邮件被滥用时给 admin 发邮件告警是循环依赖；v1 用数字面板 + CF 既有的 Origin Error 告警覆盖发现路径。

**IP 封禁完全交给 Cloudflare 控制台。** 否决：CF 免费规则位稀缺，事件响应时 admin 本就在后台里，且应用层封禁看到的是与限流相同的可信 client-IP。

## Consequences

运营者在 admin 控制台内拥有完整的「发现 → 评估 → 处置」闭环：风控面板给出信号，全部杠杆免重启即时生效（maintenance 档位、邮件总闸、预算、PoW 难度、IP 封禁）并经 `data/*.json` 跨重启保持（容器文件系统注意事项：抹掉卷会重置控制项；生产 compose 挂载 `data/`，部署不丢）。

代价与值得记住的不变量：

- 全部运行态是单机内存 + 文件；横向扩展需迁共享存储（与此前 PoW/lockout 的表述一致）。
- 封禁匹配在锁 poison 时 fail-open —— 可用性优先的取舍。
- 面板 429 数字重启归零并如实标注；OTel 计数器才是持久记录。
- 清理守卫由 SQL 固化（`email_verified = false AND is_superuser = false`、`created_at` 锚点）并被集成测试钉死；削弱任一守卫会重新引入抢占续命或误删真实账号的风险。
- 预算按「尝试发送」计数（名额在 SMTP 前占用）：SMTP 失败也消耗预算。接受实际投递量的低估，换取更简单的先占后发（防止并发冲过限额）。

验证方式：三个句柄的单测（翻零、持久化往返、CIDR 匹配、自覆盖）、清理守卫与风控聚合的 PG 集成测试（`tests/risk_controls_integration.rs`）、风控面板的前端 Vitest，以及驱动完整容器栈的 `e2e/specs/risk-controls.spec.ts`（总闸 503 往返、预算耗尽、难度旋钮、封禁往返、自封守卫、404 防枚举、维护态注册页）。
