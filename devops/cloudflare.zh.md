# Cloudflare onboarding (prod)

[English](cloudflare.md) | 中文

prod 环境的 Cloudflare 控制台操作（访客 → Cloudflare CDN → 直连回源 VPS 上的 Caddy，CF Origin Cert + Full-strict）。拓扑与环境表在 [README.zh.md](README.zh.md)；staging 隧道不是 Cloudflare 产品，staging 无本页步骤。仓库侧上线步骤（inventory、env.yml、vault、镜像 tag）见 [README.zh.md](README.zh.md#todo-when-stagingprod-go-live)。

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
- 把 **Origin Certificate** 复制到 VPS 的 `{{ app_path }}/certs/origin.pem`、**Private Key** 复制到 `{{ app_path }}/certs/origin.key`（默认路径 `/home/<user>/docker/wakewake/certs/`），两个文件 `chmod 600`——私钥仅展示一次。

## 5. Origin Rule (port rewrite)

- **Rules** → **Origin Rules** → **Create rule**：匹配 `http.host eq "wakewake.online"`，然后 **Destination Port** → **Rewrite to** → `8449` → **Deploy**。

## 6. Origin firewall

- VPS 防火墙仅对 Cloudflare IP 段（IPv4 + IPv6，<https://www.cloudflare.com/ips/>）放行入站 TCP `8449` 与 SSH，其余默认拒绝。
- 容器 Caddy 在应用层执行同一份放行表：`Caddyfile.prod` 对来源不在 `cloudflare_cidrs`（`group_vars/prod/env.yml`）内的连接直接 `403`。VPS 防火墙是可选的第二层——它还能省掉 403 本要耗费的 TLS 握手带宽。

## 7. Edge protections (free plan)

- **Security → WAF → Rate limiting rules**：用掉免费版仅有的 1 条规则——表达式 `starts_with(http.request.uri.path, "/api/v1/auth/")`，动作 **Block**，响应码 `429`。免费版约束：仅 1 条、仅按 IP 计数、固定 10 s 计数 / 10 s 处置窗口。定位是粗闸——Cloudflare 文档明确处置生效前超额请求可能仍到源站，精确限流仍以后端 governor 为准。
- **Bots → Bot Fight Mode 保持关闭。** 它会挑战 API / 移动端流量且无法按规则豁免，会打断 `wakewake-agent` 的 SSE 通道（非浏览器客户端）。浏览器页面的防护由限速规则与下面的挑战工具承担。
- 应急开关：**Security → Settings** 的 Under Attack mode（全站 managed-challenge 插页）；最多 5 条 WAF custom rules 可对热点路径加 managed challenge、按国家/ASN 封锁（按国家封禁走 custom rules——IP Access Rules 的国家封禁是企业版专属）。
- **Notifications**：开启 **Origin Error Rate Alert** 与 **Passive Origin Monitoring**，3Mbps 源站打满时有人被通知，而不是静默失败。

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
# A direct hit on the VPS IP (bypassing Cloudflare) must get 403.
curl -k -sD - -o /dev/null https://<vps-ip>:8449/
```

首次登录后，admin 后台登录历史（`login_events.ip_address`）必须是访客真实 IP——既不是伪造的 `6.6.6.6`，也不是 Cloudflare 边缘地址。Caddy 访问日志带 `client_ip`（真实访客）与 `cf-ray` 请求头，后端请求 span（`http_request`）带同样的 `client_ip` + `cf_ray`；三者可对同一请求在 CF、Caddy、应用三层互相对账。

## 9. Deploy and verify

- 完成仓库侧清单后执行 `ansible-playbook devops/ansible/deploy.yml -l prod`。
- 打开 `https://wakewake.online/api/v1/health` → `{"status":"ok"}`；playbook 尾部的 [`scripts/check_deploy.py`](../scripts/check_deploy.py) 会自动校验 health + git_sha。
