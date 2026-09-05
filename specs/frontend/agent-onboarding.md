# Agent onboarding and device-add gating

English | [中文](agent-onboarding.zh.md)

The user-facing agent pairing experience and the front-loading of device-add errors (mistakes are intercepted before the user invests any input, with a way forward — not a red banner after submit). Mechanisms and invariants below; implementation lives in `agents/page.tsx`, `devices/page.tsx`, `DeviceForm.tsx`, `lib/agent-command.ts`, and backend `routes/auth.rs`.

## Sources of truth and invariants

- **The single quota authority** is `domain::MAX_DEVICES_PER_USER` (`backend/crates/server/src/domain/mod.rs`). The frontend never copies the number; `UserPublic` (login / register / `GET /me` share the `from_user` constructor) carries the `limits.max_devices` projection. The server-side `QUOTA_EXCEEDED` check stays as a race backstop (concurrent adds from several clients). When `limits` is missing from a persisted session (an old session), the front-loaded quota check is skipped and the server backstop answers — the degradation is visible in the submit-time error copy, never silently fabricated.
- **Agent status semantics**: `public_key == null` ⇔ pending (never paired); key present + SSE connected ⇔ online; key present + disconnected ⇔ offline. The hard precondition for adding a device is **that the public key exists** (the frontend needs it to encrypt the MAC), not "online":
  - pending → block the add, route to the Agents page;
  - offline → allow the add, with an info bar saying "saved; syncs automatically once the agent is back" (consistent with the `projection_status` state machine).
- **MAC encryption needs a secure context** (Web Crypto is unavailable off HTTPS/localhost; see `lib/crypto.ts`). When `window.isSecureContext` is false the add is blocked with an explanation.
- **Agent-page command template slots**: the launch commands come from the three generators in `lib/agent-command.ts` (`buildAgentInstallCommand` one-liner / `buildAgentDockerCommand` docker run / `buildAgentComposeYaml` docker compose — field-for-field equivalent to the run command, with `docker compose up -d` attached as a trailing comment); `--server` takes `window.location.origin`, the pairing code is passed in by the caller, and the terminal card header switches between Linux / Docker tabs (distribution mechanism in [`backend/agent-distribution.md`](../backend/agent-distribution.md)). Inside the Docker tab the content is two stacked blocks: compose (with a "Recommended" chip and its own copy button for the full YAML) and the one-liner run (its own copy button). The commands match the README's real channels word for word — nothing invented.

## Agents page (`/agents`)

A three-state page driven by `useDefaultAgent()` (5s polling), no new backend endpoints:

- **pending (guidance)**: mental-model diagram (lucide `Cloud` / `House` / `Monitor` nodes + hairline connectors, the server↔agent segment highlighted as broken) → the launch-command terminal card (hairline header with an `sh` tab + `$` prompt lines, the landing page's visual language) → primary button "copy command" (one click copies the whole command, pairing info embedded) → a waiting row (pulse, "this page updates automatically"). The copy explains that the agent must run on an always-on machine under the same router as the devices being woken (WoL directed broadcast does not cross routers, RFC 919).
- **online (done)**: the diagram fully connected; one-line conclusion + CTA "go to devices" (dovetails with the devices-page onboarding); advanced section collapsed.
- **offline (repair)**: conclusion "connection lost" + repair guidance (check whether that machine is up; it reconnects automatically) + an entry point to rotate the pairing code (the masked code cannot re-pair; rotate is the only way to obtain the full code).
- **Advanced fold** (`<details>`, shared by all three states): pairing code (full/masked) + copy + two-step rotation (armed 3s pattern unchanged), agent public key PEM ("available after connecting" while pending), the config-file route (a minimal `config.toml` template).
- Copy goes through next-intl in all 8 locales; professional and friendly tone. Command blocks embed no natural-language comments — the single exception is the compose YAML, whose trailing `# 启动: docker compose up -d` line and the inline `network_mode: host` constraint note are deliberate parts of the copied payload (paste yields self-documenting, valid YAML).

## Devices page (`/devices`)

**The pre-flight state machine on "add device"** (fixed order, first hit intercepts):

```
agent.public_key == null        → 拦截弹窗：需要先连接 Agent [去连接 Agent →]
devices.length >= limits.max_devices → 拦截弹窗：设备已达上限（limits 缺失则跳过此判断）
!window.isSecureContext         → 拦截弹窗：需要 HTTPS 环境
其余                             → 打开表单
```

- The three intercepts share one lightweight dialog (icon + title + two lines of explanation + an action); the button is always clickable, and clicking always yields an explanation.
- A permanent quota badge `{count}/{max}` beside the title (not rendered when `limits` is missing).
- **Empty state + pending**: when the device list is empty and the agent is unpaired, the empty state is replaced by an onboarding card (① connect agent → ② add device → ③ wake remotely, current step highlighted, CTA into the Agents page).
- **Pending with existing devices** (the rare case of a reset agent): a header info bar routes to re-pairing.
- **Offline (paired)**: a header info bar (offline + relative time + "can add, pending sync" note + a link), non-blocking.
- Device cards' "agent offline" footnote row carries a jump link.
- **MAC validated on blur**: `react-hook-form` `mode: 'onTouched'` + pattern registration; a format error surfaces at blur time.
- **Submit-time backstop kept**: the server error-code mapping (`QUOTA_EXCEEDED` / `AGENT_NOT_FOUND` / `RATE_LIMITED` / `SYNCING` / field-level) is unchanged; `connectorNotReady` (the race where the agent is reset while the form is open) copy carries an inline link to `/agents`. Editing a device is unaffected by the gating (MAC locked).

## Acceptance

- The happy path (pending guidance → pairing → online CTA → add device) and every intercept state (pending / quota / insecure context / offline info bar) have component-level tests; `DeviceForm.test.tsx` covers blur-time MAC validation and the backstop links.
- Backend `/me`, login, and register responses carry `limits.max_devices` (value from the domain constant).
- i18n keys exist in all 8 locales (`en/zh/ja/ko/de/fr/es/pt`).
- The UI is verified with real Playwright screenshots across the three states and the gates.
