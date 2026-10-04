# Cloudflare onboarding (prod)

English | [中文](cloudflare.zh.md)

Cloudflare console operations for the prod environment (visitor → Cloudflare CDN → direct origin pull to the VPS Caddy over a CF Origin Cert, Full-strict). Topology and environment table live in [README.md](README.md); the staging tunnel is not a Cloudflare product, so staging has no steps on this page. Repo-side launch steps (inventory, env.yml, vault, image tags): [README.md](README.md#todo-when-stagingprod-go-live).

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
- Copy the **Origin Certificate** to `{{ app_path }}/certs/origin.pem` and the **Private Key** to `{{ app_path }}/certs/origin.key` on the VPS (default `/home/<user>/docker/wakewake/certs/`), then `chmod 600` both — the key is displayed only once.

## 5. Origin Rule (port rewrite)

- **Rules** → **Origin Rules** → **Create rule**: match `http.host eq "wakewake.online"`, then **Destination Port** → **Rewrite to** → `8449` → **Deploy**.

## 6. Origin firewall

- On the VPS firewall, allow inbound TCP `8449` only from the Cloudflare IP ranges (IPv4 + IPv6, https://www.cloudflare.com/ips/) plus SSH; default-deny everything else.
- The container Caddy enforces the same allowlist at the application layer: `Caddyfile.prod` answers `403` to any connection whose source is outside `cloudflare_cidrs` (`group_vars/prod/env.yml`). The VPS firewall is the optional second layer — it also saves the TLS-handshake bandwidth that a 403 would otherwise spend.

## 7. Edge protections (free plan)

- **Security → WAF → Rate limiting rules**: create the single rule the Free plan allows — expression `starts_with(http.request.uri.path, "/api/v1/auth/")`, action **Block**, response code `429`. Free-plan constraints: one rule, IP counting only, fixed 10 s counting / 10 s mitigation windows. Treat it as a coarse gate — Cloudflare documents that excess requests may still reach the origin before mitigation engages; the backend governor limits remain the precise layer.
- **Bots → Bot Fight Mode stays OFF.** It challenges API and mobile-app traffic with no rule-based exemption, which would break the `wakewake-agent` SSE channel (a non-browser client). Browser pages get their protection from the rate limiting rule and the challenge tools below.
- Emergency switches: **Security → Settings** holds Under Attack mode (a managed-challenge interstitial for the whole zone); up to 5 WAF custom rules can add a managed challenge on hot paths or block abusive countries/ASNs (country block via custom rules — IP Access Rules country blocking is Enterprise-only).
- **Notifications**: enable **Origin Error Rate Alert** and **Passive Origin Monitoring** so a saturated 3 Mbps origin pages someone instead of failing silently.

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
# A direct hit on the VPS IP (bypassing Cloudflare) must get 403.
curl -k -sD - -o /dev/null https://<vps-ip>:8449/
```

After the first login, the admin UI's login history (`login_events.ip_address`) must show the visitor's real IP — neither the spoofed `6.6.6.6` nor a Cloudflare edge address. Caddy access logs carry `client_ip` (real visitor) plus the `cf-ray` request header, and backend request spans (`http_request`) carry the same `client_ip` and `cf_ray`; the three corroborate one request across Cloudflare, Caddy, and the app.

## 9. Deploy and verify

- Complete the repo-side checklist, then run `ansible-playbook devops/ansible/deploy.yml -l prod`.
- Open `https://wakewake.online/api/v1/health` → `{"status":"ok"}`; the playbook's trailing check ([`scripts/check_deploy.py`](../scripts/check_deploy.py)) verifies health + git_sha automatically.
