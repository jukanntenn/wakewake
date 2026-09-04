# Risk controls (admin runtime)

English | [中文](risk-controls.zh.md)

The operator-side abuse controls behind `/admin/*`: what each lever does, the invariants that must not move, and where each piece of state lives. The decision record is [`../../.agents/wrfcs/implemented/2026-09-04-admin-risk-controls.md`](../../.agents/wrfcs/implemented/2026-09-04-admin-risk-controls.md).

## The runtime-handle family

All runtime levers follow one pattern, established by maintenance mode: in-memory truth behind an `Arc<RwLock>` handle in `AppState`, persisted best-effort to a JSON file under `data/` (restored on startup, overriding config defaults), every admin mutation audited into `admin_actions`. Wiping the `data/` volume resets controls to config defaults — the prod compose mounts `data/`, so deploys keep them.

| Handle | File | Admin API |
|---|---|---|
| `MaintenanceHandle` (`service/maintenance.rs`) | `data/maintenance.json` | `GET/POST /admin/maintenance` |
| `MailerControl` (`service/mailer_control.rs`) | `data/mailer.json` | `GET/POST /admin/mailer` |
| `PowService` difficulty knob (`service/pow.rs`) | `data/pow.json` | `GET/POST /admin/pow` |
| `IpBanStore` (`service/ip_ban.rs`) | `data/ip_bans.json` | `GET/POST/DELETE /admin/ip-bans` |

Registration closure is the maintenance `registration_disabled` mode — there is deliberately no separate switch (readonly/full already imply closure; one source of truth for "is registration open"). Audience rules: the register page disables its form by polling public `/health/maintenance` (any enabled mode blocks registration); `MaintenanceBanner` renders only in `readonly` mode, because its signed-in audience is unaffected by registration closure.

## Email budgets

Three send paths account independently per UTC day: `register` (verification email on signup — the only path where the attacker picks arbitrary recipients), `resend`, `reset`. Limits default to 500/200/300 (`[mailer].max_*_emails_per_day`, 0 = unlimited) and are runtime-adjustable.

Invariants:

- **Budget exhaustion degrades exactly like `mailer.enabled = false`**: registration proceeds silently (account stays unverified, resend can recover it later), resend is silently skipped (always-200 anti-enumeration), password-reset requests return 503 at the route's pre-check (`would_send`). No dedicated error code exists for exhaustion on purpose.
- **Counters persist across restarts** — an attacker cannot wait out a budget with a restart, and neither can an operator accidentally reset it.
- A slot (`try_acquire`) is consumed **before** the SMTP attempt; SMTP failures therefore count against the budget. The non-consuming peek `would_send` exists for route-level pre-checks and admin display.
- Rejections are never silent: a `blocked` counter per path, `emails_blocked_total{path,reason}` in OTel, and warn logs.
- `verification_sent_at` (the 60s resend cooldown anchor) is written only on a successful send — failures and exhausted budgets do not burn the cooldown.

## PoW difficulty

`0..=10`, default 4 (config `pow.difficulty`). Each challenge embeds the difficulty current at issuance and is verified against its own value — turning the knob never invalidates in-flight challenges (the 10min TTL is the natural transition window). Difficulty 0 disables the proof entirely; treat it as a debugging affordance, not a production state.

## Unverified-account purge

Daily housekeeping (24h interval, first tick at startup) deletes `users` where `email_verified = false AND is_superuser = false AND created_at < now() - unverified_retention_days` (default 7, 0 = off). FK cascades remove the 1:1 agent row and any refresh tokens.

Invariants:

- The age anchor is **`created_at`, never `verification_sent_at`**: the resend endpoint is anti-enumeration always-200, so anyone can refresh the latter daily to keep a squatted address alive.
- The SQL guards (`email_verified = false`, `is_superuser = false`) are belt-and-braces against ever touching real accounts; both are pinned by integration tests.
- Unverified users cannot own devices/integrations (login is blocked), so the `ON DELETE RESTRICT` on `devices.agent_id`/`integrations.agent_id` is unreachable on this path.
- Deletes run in batches of 1000 so the first post-flood purge is not one giant transaction.

## IP bans

Entries are an exact IP (HashMap lookup) or a CIDR prefix (linear scan over the handful of entries), with an optional TTL; expiry is judged lazily on read and expired entries are compacted by the daily housekeeping. Matching uses the same client-IP derivation as the rate limiter (`util::client_ip_from_headers`), which is only spoof-proof because Caddy 403s non-Cloudflare direct connections — the ban layer inherits that topology contract.

Invariants:

- The middleware mounts **outside** the global rate limit (banned IPs don't consume governor budget); `/api/v1/health*` stays exempt (probes must never carry business semantics).
- **Fail-open**: on lock poisoning the middleware passes requests through — the ban layer must not be able to take the site down by failing.
- The **self-ban guard** rejects any target that would cover the requester's own IP (422 `self_ban` field code) — the alternative is an operator locked out of the only interface that can undo the ban.
- The same task that purges unverified accounts also compacts expired bans.

## Risk panel

`GET /admin/risk` composes three source kinds: DB aggregation over `users`/`login_events` (registrations 24h/7d, unverified backlog + oldest age, failed logins, top failed IPs with distinct-email counts, top targeted emails), runtime-handle snapshots (mailer counters, PoW difficulty, active ban count), and an in-process 429 counter exposed as `rate_limited_since_start` + uptime. The 429 number **resets on restart and is labeled as such**; OTel counters and CF analytics are the durable records. The admin overview renders this as a Risk section whose per-IP **Ban** button defaults to a 24h TTL — fast response with self-healing for false positives; promote to permanent in the ban list only for confirmed long-term abuse.
