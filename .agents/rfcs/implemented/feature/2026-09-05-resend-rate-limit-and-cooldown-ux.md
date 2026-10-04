# RFC: Email-resend rate-limit group and honest cooldown UX

Status: implemented

English | [中文](2026-09-05-resend-rate-limit-and-cooldown-ux.zh.md)

## Problem

The verification-email resend endpoint (`POST /auth/verify-email/resend`) was mounted on the password-reset governor group: 3 requests/hour per client IP (GCRA burst 3, one permit back every 20 minutes). Three protection layers with mismatched horizons governed one button:

1. Service layer: per-user 60 s cooldown, silent always-200 (anti-enumeration) — never emits 429.
2. Middleware: per-IP 3/hour (the only possible 429 source).
3. Frontend: any 429 rendered as a fixed "please wait a minute" toast; `Retry-After` was never parsed.

A real user whose email is slow clicks roughly once a minute. Each click burns an IP permit even when the service layer silently no-ops it, so three clicks exhaust the bucket — and every subsequent click for up to 20 minutes returns 429 with a toast claiming one minute. The user waits "well over a minute", clicks again, and gets the same message: the reported symptom. The frontend also offered no countdown, actively encouraging the repeated clicks that exhaust the bucket.

## Decision

- **Resend moves to its own governor group: per-IP 6/hour** (`email_resend_rate_limit_layer`, `middleware/rate_limit.rs`), decoupled from password-reset's 3/hour. Anti-bombing remains three-layered: per-user 60 s cooldown, per-user 200 resend emails/day budget (`[mailer].max_resend_emails_per_day`), per-IP 6/hour.
- **The check-email page disables the resend button with a live countdown**: 60 s from mount (mount time ≈ send time right after registration, aligned with the service-layer cooldown), reset to 60 s after each send; on 429 the countdown takes `max(60, Retry-After)`.
- **`ApiError` parses `Retry-After`** (delta-seconds per RFC 9110 §10.2.3; governor always emits that form) into `retryAfterSeconds`, and the 429 toast states the true wait instead of a fixed "one minute".
- The always-200 anti-enumeration semantics of the service layer are unchanged; the IP-level 429 fires before the handler and leaks nothing about account existence.

## Alternatives considered

- **Keep 3/hour, fix only the messaging** — rejected: honest messaging makes the lockout visible but still punishes a legitimate eager user with up to 20 silent minutes; the per-user layers already bound per-account bombing, so the IP layer's only unique job is cross-account enumeration, which 6/hour still bounds.
- **No IP-level limiter on resend at all** — rejected: it is the only cross-account bound for an attacker registering many victim-addressed accounts (register itself is PoW + 10/min/IP) and triggering resends from one IP.
- **Return the service-layer cooldown as an explicit 429** — rejected: it would break the anti-enumeration contract (a 429 tied to account state would distinguish existing unverified accounts from unknown emails).

## Consequences

- A NAT'd household or office shares the 6/hour bucket; six resends within an hour across all users behind one IP then wait for permits. The countdown and true `Retry-After` messaging make that state visible instead of mysterious.
- The countdown constant (60 s) is duplicated frontend-side (`check-email/page.tsx`) and backend-side (`RESEND_COOLDOWN_SECS`); they are coupled by comment, not by config — acceptable for a display approximation that never gates anything server-side.
