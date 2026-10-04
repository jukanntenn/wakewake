# Cloudflare onboarding (prod)

English | [中文](cloudflare.zh.md)

Cloudflare console operations for the prod environment. Topology (host gateway): visitor → Cloudflare CDN (orange cloud, Full-strict) → host systemd Caddy on `:443` (terminates TLS with a CF Origin Cert, admits only Cloudflare CIDR sources) → `127.0.0.1:8449` (container Caddy, plain HTTP). The container port is loopback-only; the host gateway is the single public entry. Why a host gateway instead of direct origin pull on a high port: on the Free plan only proxied port 443 has edge caching — every other port answers `CF-Cache-Status: DYNAMIC`, which would switch the whole zone (immutable frontend assets included) to uncached. Topology and environment table live in [README.md](README.md); the staging tunnel is not a Cloudflare product, so staging has no steps on this page. Repo-side launch steps (inventory, env.yml, vault, image tags): [README.md](README.md#todo-when-stagingprod-go-live).

Prerequisites: a Cloudflare account; `wakewake.online` registered at a registrar; the VPS public IPv4 address.

## 1. Add the zone

- Dashboard → **Onboard a domain** → enter `wakewake.online` → **Continue** → select the **Free** plan.
- Review the scanned DNS records → **Continue** (the record this deployment needs is created in the next step).
- At the registrar, replace the nameservers with the two assigned `<name>.ns.cloudflare.com` hosts (turn off registrar DNSSEC first if it is active; re-enable DNSSEC from DNS → Settings once the zone is Active).
- Wait for the zone status to become **Active** (email notification).

## 2. DNS record

- **DNS** → **Records** → **Create record**: Type `A`, Name `@`, IPv4 address = VPS public IP, Proxy status **Proxied** (orange cloud) → **Save**.

## 3. SSL/TLS mode

- **SSL/TLS** → **Overview** → set the encryption mode to **Full (strict)**.
- **SSL/TLS** → **Edge Certificates** → enable **Always Use HTTPS**.

## 4. Origin Certificate

- **SSL/TLS** → **Origin Server** → **Origin Certificates** tab → **Create Certificate** → keep **Generate with Cloudflare** (RSA) and the default hostnames (`wakewake.online` + `*.wakewake.online`) → pick the longest validity offered → Key Format **PEM** → **Create**.
- Copy the **Origin Certificate** to `{{ app_path }}/certs/origin.pem` and the **Private Key** to `{{ app_path }}/certs/origin.key` on the VPS (default `/home/<user>/docker/wakewake/certs/`), `chmod 600` both — the key is displayed only once. `deploy.yml` installs both into `/etc/caddy/certs/wakewake/` (root:caddy) and the gateway site block reads them there; the `certs/` copies are the durable source deploy runs from.

## 5. Host gateway (site block)

Cloudflare connects on standard 443, where the shared host Caddy listens (`import /etc/caddy/conf.d/*.caddy`; no Cloudflare Origin Rule is involved). `deploy.yml -l prod -K` renders the wakewake site block ([`wakewake.caddy.j2`](ansible/templates/wakewake.caddy.j2)) into `/etc/caddy/conf.d/wakewake.caddy` and reloads Caddy:

- `tls` with the Origin Cert pair from §4 — Full-strict origin pull.
- `@not_cf not remote_ip <cloudflare_cidrs>` → **403**: any connection that bypasses Cloudflare (forged SNI straight to the VPS IP) is rejected.
- `reverse_proxy 127.0.0.1:8449` — the loopback-only container publish; `CF-Connecting-IP` passes through untouched for the backend's authoritative client IP.

## 6. Origin lockdown

- App layer (always on): the gateway site block from §5 is the enforcement — bypassing CF means bypassing every edge protection (rate limiting, WAF, caching), so non-CF sources get 403.
- Host firewall (optional second layer): on a dedicated VPS, allow inbound 443 only from the Cloudflare IP ranges (https://www.cloudflare.com/ips/). Not applicable on abj — 443 is shared with other production sites; the site-block guard is the layer there.
- VPS security group: 443 must be open (it already is for the co-hosted sites).

## 7. Edge protections (free plan)

- **Rules → Cache Rules → Create rule** `wakewake-shell`: match `(not starts_with(http.request.uri.path, "/api/")) and (not starts_with(http.request.uri.path, "/_next/static/"))` → **Eligible for cache**, Edge TTL override **5 minutes**, Browser TTL **Respect Existing Headers**. Immutable `/_next/static/*` assets cache by default (origin sends `max-age=31536000, immutable`); this rule adds a short edge TTL for the HTML shell, whose origin `Cache-Control` is `max-age=0, must-revalidate` — browsers revalidate every navigation while the edge stops re-pulling the shell from the origin within the TTL window.
- **Security → WAF → Rate limiting rules**: create the single rule the Free plan allows — expression `starts_with(http.request.uri.path, "/api/v1/auth/")`, action **Block**, response code `429`. Free-plan constraints: one rule, IP counting only, fixed 10 s counting / 10 s mitigation windows. Treat it as a coarse gate — Cloudflare documents that excess requests may still reach the origin before mitigation engages; the backend governor limits remain the precise layer.
- **Bots → Bot Fight Mode stays OFF.** It challenges API and mobile-app traffic with no rule-based exemption, which would break the `wakewake-agent` SSE channel (a non-browser client). Browser pages get their protection from the rate limiting rule and the challenge tools below.
- Emergency switches: **Security → Settings** holds Under Attack mode (a managed-challenge interstitial for the whole zone); up to 5 WAF custom rules can add a managed challenge on hot paths or block abusive countries/ASNs (country block via custom rules — IP Access Rules country blocking is Enterprise-only).
- **Notifications**: enable **Origin Error Rate Alert** and **Passive Origin Monitoring** so a saturated origin pages someone instead of failing silently.

## 8. Post-launch verification

Cache contract (`curl -sD -` and read the response headers):

```bash
# Immutable assets: first request MISS, second HIT, Age grows on hits,
# origin Cache-Control "public, max-age=31536000, immutable" preserved.
curl -sD - -o /dev/null https://<domain>/_next/static/chunks/<file>.js
# HTML shell: short edge TTL per the Cache Rule, cache-control max-age=0 must-revalidate.
curl -sD - -o /dev/null https://<domain>/
# API: cf-cache-status DYNAMIC, no Age header.
curl -sD - -o /dev/null https://<domain>/api/v1/health
# SSE: connection stays open, one ": ping" comment every 30 s.
curl -sN https://<domain>/api/v1/agents/self/events
```

IP chain:

```bash
# A spoofed XFF through Cloudflare must NOT change the recorded client IP.
curl -H 'X-Forwarded-For: 6.6.6.6' -sD - -o /dev/null https://<domain>/api/v1/health
# A direct hit on the VPS IP (forged SNI, bypassing Cloudflare) must get 403.
curl --resolve <domain>:443:<vps-ip> -sD - -o /dev/null https://<domain>/
```

After the first login, the admin UI's login history (`login_events.ip_address`) must show the visitor's real IP — neither the spoofed `6.6.6.6` nor a Cloudflare edge address. Backend request spans (`http_request`) carry the same `client_ip` (from `CF-Connecting-IP`, which the gateway passes through untouched) plus `cf_ray` — the two corroborate one request across Cloudflare and the app.

## 9. Deploy and verify

- VPS prerequisites (provisioned by ops, not by the playbook): Docker Engine with the compose plugin, the deploy user in the `docker` group, and the shared host Caddy (`import /etc/caddy/conf.d/*.caddy`) listening on 443.
- Complete the Cloudflare steps above (the DNS record must be live — the trailing health check goes through `https://wakewake.online`), then run `ansible-playbook devops/ansible/deploy.yml -l prod -K` (the `-K` sudo password feeds the gateway tasks: origin-cert install, site block, Caddy reload).
- Open `https://wakewake.online/api/v1/health` → `{"status":"ok"}`; the playbook's trailing check ([`scripts/check_deploy.py`](../scripts/check_deploy.py)) verifies health + git_sha automatically.
