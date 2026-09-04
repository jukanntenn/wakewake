# WRFC: Admin runtime risk controls

Status: implemented

English | [中文](2026-09-04-admin-risk-controls.zh.md)

## Problem

Preparing for global public operation exposed an operator-side gap: the product had thick per-request defenses (PoW, per-IP governor limits, login lockout, anti-enumeration) but no in-band levers for the abuse scenarios that actually burn a 2 GB VPS:

- **Email spend was unbounded and uncontrollable at runtime.** Registration is the only send path where the attacker picks arbitrary recipient addresses; distributed IPs defeat per-IP limits and PoW difficulty 4 is millisecond-cheap for bots. Beyond SMTP quota, spam-trap hits destroy sender reputation and can get the SMTP account suspended — which kills password reset for real users, an auth-wide DoS. `mailer.enabled` was frozen at startup, so the only response mid-attack was a process restart.
- **Unverified accounts accumulate forever.** Each registration writes a `users` row plus a 1:1 `agents` row; nothing ever deletes them. Registration floods permanently pollute the DB, and a squatter who registers a victim's email without verifying blocks the victim with 409 `USER_EXISTS` indefinitely.
- **No application-layer IP ban.** The only ban path was the Cloudflare console — out-of-band, and CF Free's five WAF rules are too scarce to spend on individual IPs.
- **Abuse was invisible.** Registration rate, email spend, failed-login aggregation, and 429 counts had no admin surface; discovery depended on CF alerts or log spelunking.
- The registration kill switch already existed (maintenance `registration_disabled`) but leaked UX gaps: the register page let users solve PoW before hitting a 403 toast, and the dashboard-wide warning banner targeted the wrong audience (signed-in users, whom registration closure does not affect).

## Decision

Runtime controls follow the `MaintenanceHandle` pattern — in-memory truth behind an `Arc<RwLock>` handle, persisted to a JSON file under `data/`, restored at startup, every admin mutation audited into `admin_actions`. Four handles/controls shipped:

1. **Registration switch stays in maintenance** (`registration_disabled` mode; single source of truth — `readonly`/`full` already imply closure). Polish: the register page polls public `/health/maintenance` and disables the form with the announcement instead of letting users burn a PoW solve; `MaintenanceBanner` renders only for `readonly` (registration closure does not affect its signed-in audience).
2. **`MailerControl`** (`service/mailer_control.rs`, `data/mailer.json`): a runtime kill switch plus per-path UTC-daily budgets split three ways — `register`/`resend`/`reset` (defaults 500/200/300, 0 = unlimited). Split budgets are the root fix: a registration flood cannot drain the password-reset share. Budget exhaustion degrades each path exactly as `mailer.enabled=false` does today (register silently skips, resend is silent, reset request returns 503) — zero new error codes or user-facing UX. Counters persist, so a restart does not reopen the spigot. `MailerService` acquires a slot before sending (`try_acquire`); `would_send` is the non-consuming peek used by the reset route's 503 pre-check.
3. **PoW difficulty is a runtime knob** (`service/pow.rs`, `data/pow.json`, `GET/POST /admin/pow`, 0..=10): raising 4 → 6–7 multiplies bot cost 256–4096× — a middle gear between "open" and "closed". Each challenge embeds its issuance-time difficulty, so turning the knob never invalidates in-flight challenges.
4. **`IpBanStore`** (`service/ip_ban.rs`, `data/ip_bans.json`): exact-IP (HashMap) and CIDR entries (linear scan — tens of entries), TTL or permanent, expiry judged lazily on read, compacted by the daily housekeeping task. The middleware (`middleware/ip_ban.rs`) mounts outside the global rate limit (banned IPs don't consume governor budget), health routes stay exempt, and it fails open — a broken ban layer must not take the site down. IP trust rests on the existing topology: Caddy 403s non-Cloudflare direct connects, so the client-IP source shared with rate limiting cannot be spoofed. A self-ban guard rejects any target covering the requester's own IP (422), preventing the one-way lockout of banning oneself.
5. **Unverified purge** (`user_repo::purge_unverified`, `security.unverified_retention_days`, default 7, 0 = off): daily batch-deleted where `email_verified = false AND is_superuser = false AND created_at < now() - N days`; FK cascades clean `agents`/`refresh_tokens`. The anchor is `created_at`, never `verification_sent_at` — the resend endpoint is anti-enumeration always-200, so any third party could refresh that timestamp daily to keep a squatted account alive. Batching (1000/iteration) keeps the first post-flood purge out of big-transaction territory.
6. **`GET /admin/risk`** aggregates DB signals (registrations 24h/7d, unverified backlog + age, failed logins, top failed IPs / top targeted emails from `login_events`), runtime-handle snapshots (mailer, PoW, ban count), and an in-process 429 counter (readable via `metrics::rate_limited_snapshot`; restart resets it — labeled as such, CF analytics remains the reconciliation source). The admin overview page renders this as a "Risk" section with one-click **Ban** (default 24h TTL) on top failed IPs. New OTel counters: `emails_total{path}`, `emails_blocked_total{path,reason}`, `unverified_purged_total`, `http_rate_limited_total`.

Two adjacent defects fixed in passing: verification-resend now records `verification_sent_at` only on successful send (aligning with reset's deliberate semantics — failures no longer burn the 60s cooldown), and the mailer 503 pre-check moved from `is_enabled()` to `would_send(Reset)` so the runtime gate participates.

## Alternatives considered

**A separate first-class registration switch (own API/persistence).** Rejected: `readonly`/`full` already imply registration closure, so an independent flag adds no capability while creating a second source of truth for "is registration open"; the maintenance page already labels the mode "Pause registration", not maintenance jargon.

**Kill switch only (no budgets).** Rejected as incomplete: with a single switch, closing email to stop a registration flood also 503s password reset for real users — the attacker achieves the auth-DoS by forcing the operator's hand. Split budgets are precisely the mechanism that removes that coupling.

**Global email budget instead of per-path.** Rejected: a single pool lets registration garbage exhaust the reset share — the same DoS through the budget itself.

**Counters in the database.** Rejected: bans and budgets are operational runtime state, not business data; the file-backed handle avoids a migration and matches the established `maintenance.json` precedent.

**DB-backed IP ban table.** Rejected for the same reason; the store is memory-resident for the hot path, the file is only persistence.

**Admin manual "purge now" button.** Rejected: the scheduled task is the root fix; during a flood the 24h difference is immaterial, and the button adds attack surface API for no operational value.

**Alerting thresholds / automated notifications.** Deferred: email-to-admin is circular when email is the abused resource; the numbers panel plus CF's existing Origin Error alert cover discovery for v1.

**Defer IP bans entirely to the Cloudflare console.** Rejected: CF Free's rule slots are scarce, the admin already lives in the dashboard during incidents, and app-level bans see the same trusted client IP the rate limiter sees.

## Consequences

The operator now has a complete notice → assess → act loop inside the admin console: risk panel signals, and levers that take effect without restart (maintenance modes, mailer gate, budgets, PoW difficulty, IP bans) and survive restarts via `data/*.json` (a container FS caveat: wiping the volume resets controls; the prod compose mounts `data/`, so deploys keep them).

Costs and invariants worth remembering:

- All runtime state is single-node in-memory + file; horizontal scaling would move it to shared storage (same statement as PoW/lockout before it).
- Ban matching is fail-open on lock poisoning by design — an availability-first trade-off.
- The 429 panel number resets on restart and is labeled accordingly; OTel counters are the durable record.
- Purge invariants are guarded in SQL (`email_verified = false AND is_superuser = false`, `created_at` anchor) and pinned by integration tests; weakening either guard would reintroduce the squatting keep-alive or risk deleting real accounts.
- Budget accounting counts *attempted* sends (slot acquired before SMTP): SMTP failures consume budget. Under-counting actual delivery is accepted for the simpler pre-acquire that prevents racing past the limit.

Verified by: unit tests for all three handles (rollover, persistence round-trip, CIDR matching, self-cover), PG integration tests for purge guards and risk aggregation (`tests/risk_controls_integration.rs`), frontend Vitest for the risk panel, and `e2e/specs/risk-controls.spec.ts` driving the full container stack (gate 503 round-trip, budget exhaustion, knob, ban round-trip, self-ban guard, 404 anti-enumeration, maintenance-gated register page).
