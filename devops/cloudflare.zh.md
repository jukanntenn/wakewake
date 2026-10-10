# Cloudflare onboarding (prod)

[English](cloudflare.md) | 中文

prod 环境的 Cloudflare 控制台操作。拓扑（宿主网关型）：访客 → Cloudflare CDN（橙云、Full-strict）→ 宿主 systemd Caddy `:443`（CF Origin Cert 终结 TLS，仅放行 CF IP 段来源）→ `127.0.0.1:8449`（容器 Caddy，plain HTTP）。容器端口仅回环发布，对外唯一入口是宿主网关。为什么用宿主网关而非高端口直连回源：免费套餐下只有 443 端口的代理流量有边缘缓存——其余端口一律 `CF-Cache-Status: DYNAMIC`，等于全站（含前端 immutable 静态资源）不可缓存。拓扑与环境表在 [README.zh.md](README.zh.md)；staging 隧道不是 Cloudflare 产品，staging 无本页步骤。仓库侧上线步骤（inventory、env.yml、vault、镜像 tag）见 [README.zh.md](README.zh.md#todo-when-stagingprod-go-live)。

前置条件：Cloudflare 账号；已在注册商注册 `wakewake.online`；VPS 公网 IPv4 地址。

## 1. Add the zone

- 控制台 → **Onboard a domain** → 输入 `wakewake.online` → **Continue** → 选择 **Free** 套餐。
- 检查扫描到的 DNS 记录 → **Continue**（本部署所需记录在下一步创建）。
- 在注册商处把 nameservers 换成分配的两个 `<name>.ns.cloudflare.com`（若注册商侧 DNSSEC 开启则先关闭；zone Active 后再到 DNS → Settings 重新开启 DNSSEC）。
- 等待 zone 状态变为 **Active**（邮件通知）。

## 2. DNS record

- **DNS** → **Records** → **Create record**：Type `A`、Name `@`、IPv4 address 填 VPS 公网 IP、Proxy status 选 **Proxied**（橙色云朵）→ **Save**。

## 3. SSL/TLS mode

- **SSL/TLS** → **Overview** → 加密模式设为 **Full (strict)**。
- **SSL/TLS** → **Edge Certificates** → 开启 **Always Use HTTPS**。

## 4. Origin Certificate

- **SSL/TLS** → **Origin Server** → **Origin Certificates** 页签 → **Create Certificate** → 保持 **Generate with Cloudflare**（RSA）与默认主机名（`wakewake.online` + `*.wakewake.online`）→ 有效期选可选项中最长档 → Key Format 选 **PEM** → **Create**。
- 把 **Origin Certificate** 复制到 VPS 的 `{{ app_path }}/certs/origin.pem`、**Private Key** 复制到 `{{ app_path }}/certs/origin.key`（默认路径 `/home/<user>/docker/wakewake/certs/`），两个文件 `chmod 600`——私钥仅展示一次。`deploy.yml` 会把两者安装到 `/etc/caddy/certs/wakewake/`（root:caddy），网关 site 块从那里读取；`certs/` 下的副本是每次部署的持久来源。

## 5. Host gateway（网关 site 块）

Cloudflare 走标准 443 回源，宿主 Caddy 在 443 监听（`import /etc/caddy/conf.d/*.caddy`，不涉及任何 Cloudflare Origin Rule）。`deploy.yml -l prod -K` 把 wakewake 的 site 块（[`wakewake.caddy.j2`](ansible/templates/wakewake.caddy.j2)）渲染进 `/etc/caddy/conf.d/wakewake.caddy` 并 reload Caddy：

- `tls` 用 §4 的 Origin Cert 证书对——CF Full-strict 回源。
- `@not_cf not remote_ip <cloudflare_cidrs>` → **403**：绕过 Cloudflare 直连 VPS IP（伪造 SNI）的连接一律拒绝。
- `reverse_proxy 127.0.0.1:8449`——容器端口仅回环发布；`CF-Connecting-IP` 头原样透传，作为后端的权威客户端 IP。

## 6. Origin lockdown

- 应用层（恒开）：§5 的网关 site 块就是执行点——绕过 CF 即绕过全部边缘防护（限速/WAF/缓存），非 CF 来源一律 403。
- 宿主防火墙（可选第二层）：独占 VPS 可将入站 443 仅对 Cloudflare IP 段（<https://www.cloudflare.com/ips/>）放行。abj 不适用——443 与其他生产站点共用，site 块守卫即该主机的执行层。
- VPS 安全组：443 必须放行（共存站点早已放行）。

## 7. Edge protections (free plan)

- **Rules → Cache Rules → Create rule** `wakewake-shell`：匹配 `(not starts_with(http.request.uri.path, "/api/")) and (not starts_with(http.request.uri.path, "/_next/static/"))` → **Eligible for cache**、Edge TTL 覆写 **5 minutes**、Browser TTL **Respect Existing Headers**。`/_next/static/*` immutable 资源默认即可缓存（源站发 `max-age=31536000, immutable`）；本规则给 HTML 外壳补一个短边缘 TTL——源站外壳是 `max-age=0, must-revalidate`，浏览器每次导航仍条件请求，边缘则在该窗口内不再回源拉外壳。
- **Security → WAF → Rate limiting rules**：用掉免费版仅有的 1 条规则——表达式 `starts_with(http.request.uri.path, "/api/v1/auth/")`，动作 **Block**，响应码 `429`。免费版约束：仅 1 条、仅按 IP 计数、固定 10 s 计数 / 10 s 处置窗口。定位是粗闸——Cloudflare 文档明确处置生效前超额请求可能仍到源站，精确限流仍以后端 governor 为准。
- **Bots → Bot Fight Mode 保持关闭。** 它会挑战 API / 移动端流量且无法按规则豁免，会打断 `wakewake-agent` 的 SSE 通道（非浏览器客户端）。浏览器页面的防护由限速规则与下面的挑战工具承担。
- 应急开关：**Security → Settings** 的 Under Attack mode（全站 managed-challenge 插页）；最多 5 条 WAF custom rules 可对热点路径加 managed challenge、按国家/ASN 封锁（按国家封禁走 custom rules——IP Access Rules 的国家封禁是企业版专属）。
- **Notifications**：开启 **Origin Error Rate Alert** 与 **Passive Origin Monitoring**，源站打满时有人被通知，而不是静默失败。

## 8. Post-launch verification

缓存契约（`curl -sD -` 看响应头）：

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

IP 链路：

```bash
# A spoofed XFF through Cloudflare must NOT change the recorded client IP.
curl -H 'X-Forwarded-For: 6.6.6.6' -sD - -o /dev/null https://<domain>/api/v1/health
# A direct hit on the VPS IP (forged SNI, bypassing Cloudflare) must get 403.
# -k is required: the origin presents the CF Origin Cert (Origin-CA-signed,
# not publicly trusted), so client-side verification fails before the guard.
curl -k --resolve <domain>:443:<vps-ip> -sD - -o /dev/null https://<domain>/
```

首次登录后，admin 后台登录历史（`login_events.ip_address`）必须是访客真实 IP——既不是伪造的 `6.6.6.6`，也不是 Cloudflare 边缘地址。后端请求 span（`http_request`）带同一个 `client_ip`（来自网关原样透传的 `CF-Connecting-IP`）与 `cf_ray`；两者可对同一请求在 Cloudflare 与应用两层互相对账。

## 9. Deploy and verify

- VPS 前置（运维 provisioning，本 playbook 不安装）：Docker Engine + compose 插件、部署用户在 `docker` 组、宿主 Caddy（`import /etc/caddy/conf.d/*.caddy`）在 443 监听。
- 先完成上面的 Cloudflare 步骤（DNS 记录必须已生效——部署尾部的健康校验走 `https://wakewake.online`），然后执行 `ansible-playbook devops/ansible/deploy.yml -l prod -K`（`-K` 的 sudo 密码供网关任务：Origin Cert 安装、site 块、Caddy reload）。
- 打开 `https://wakewake.online/api/v1/health` → `{"status":"ok"}`；playbook 尾部的 [`scripts/check_deploy.py`](../scripts/check_deploy.py) 会自动校验 health + git_sha。
