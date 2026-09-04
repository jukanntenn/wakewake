# Agent distribution (one-line install)

English | [中文](agent-distribution.zh.md)

The acquisition, configuration, and persistence path of the agent for non-technical users. Design decisions (terminal state): dual-channel distribution (GitHub Releases binaries + Docker Hub production image), an install.sh one-liner, a `service install` subcommand for persistence, and `tls.ca_cert` covering the self-signed case. Implementation lives in the repo-root `install.sh`, `backend/crates/agent/src/service.rs`, `.github/workflows/release.yml`, `docker/agent-prod.Dockerfile`, and the frontend `lib/agent-command.ts`.

## Channel and asset matrix

- **Binaries**: GitHub Releases (tag `v*` triggers `release.yml`, taiki-e/upload-rust-binary-action), three musl **statically** linked targets (immune to old NAS firmware glibc):
  - `x86_64-unknown-linux-musl` (x86 NAS / mini PCs)
  - `aarch64-unknown-linux-musl` (Pi 3/4/5, ARM NAS)
  - `armv7-unknown-linux-musleabihf` (old Pi / 32-bit ARM NAS; armv6 unsupported)
- **Asset naming** is `$bin-$target.tar.gz` (**no version in the name**; taiki-e `archive: $bin-$target`) → `releases/latest/download/<asset>` is a cross-version stable link, so install.sh needs no GitHub API tag resolution. Each target ships a `<archive>.sha256` (standard `sha256sum` format).
- **Docker image**: `jukanntenn/wakewake-agent` (Docker Hub, same repo pattern as the main image). The production image is `docker/agent-prod.Dockerfile` (**single-process entrypoint**, no s6/sniffer) — a separate build from the E2E-only `docker/agent.Dockerfile` (danger feature + dual process); the two are never reused for each other. Publishing goes through the agent job of `docker-publish.yml` (same tag trigger, native arm runner + imagetools merge).
- **Windows / macOS**: planned, noted in the README. In the WoL scenario a Windows machine is a wake target, not an agent host; always-on hosts are naturally Linux NAS/Pi/mini PCs.
- **Version compatibility promise**: install.sh always pulls latest; `protocol` crate fields are add-only (the state snapshot already carries a version watermark), so agent and server versions are decoupled.

## install.sh contract (repo root, POSIX sh)

The one-liner the UI generates (arguments via `sh -s --`, avoiding quoting hell):

```sh
curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh | sh -s -- --server <origin> --code <配对码>
```

Script steps (fail fast, `set -eu`):

1. Parse `--server` / `--code` (both `--x=y` and `--x y` forms); missing arguments print usage and exit.
2. Platform detection: `uname -s` must be Linux; `uname -m` maps `x86_64|amd64`→x86_64 target, `aarch64|arm64`→aarch64, `armv7l|armv8l`→armv7; `armv6l`/others print "not supported yet" with the support matrix.
3. Download `releases/latest/download/wakewake-agent-<target>.tar.gz` + `.sha256`; **compare the hash byte by byte** (no reliance on `sha256sum -c`; busybox/toybox compatible).
4. Unpack and install: as root (with `/usr/local/bin` writable) into `/usr/local/bin/wakewake-agent`; otherwise into `~/.local/bin` with a PATH hint.
5. Write config: `config.toml` under `$WAKEWAKE_HOME` (default `~/.wakewake`) — **if one exists, back it up first as `config.toml.bak.<timestamp>`** (key.pem and user-customized fields cannot be merged, so backup is the only safe strategy); write the minimal `server_url` + `pairing_code`, `chmod 600` (the file holds the pairing code). `key.pem` is never touched.
6. **Foreground exec start** (`exec $BIN --config <cfg>`): the user immediately sees connection logs in the terminal and the agents page turning green; before starting, print the persistence hint (`sudo wakewake-agent service install`).

Idempotency: re-running re-downloads/overwrites the binary and rewrites the config after backing it up — after a pairing-code rotation, re-running the same one-liner completes re-pairing.

## Persistent run: the `service install` subcommand

The inevitable consequence of a foreground run: the agent dies with the ssh session. Persistence = `sudo wakewake-agent service install` (the cloudflared pattern):

- **Linux systemd only**: non-Linux errors out and points to the Docker route; a missing `systemctl` errors the same way.
- **root required**: euid is decided by parsing the first `Uid:` field of `/proc/self/status`; non-root prints "sudo required".
- **User semantics**: the unit runs as `User=<SUDO_USER or current user>` (under `sudo` the service still runs as the original user, file permissions identical to the foreground run); HOME is resolved through `getent passwd` to that user's real home (falling back to `/home/<user>`) and set explicitly via `Environment=HOME=` in the unit — config search and `home_dir` (key.pem/logs) behave exactly as in the foreground.
- **Config location**: `--config` explicitly passes `<WAKEWAKE_HOME>/config.toml` (or a CLI `--config`); a missing file errors and guides the user to run install.sh first / create the config (a service never starts silently on a wrong path).
- **Unit content** (template in `service.rs`, testable): `After/Wants=network-online.target`, `Restart=on-failure` + `RestartSec=5`, `ExecStart=<current_exe> --config <cfg>`. Paths containing spaces get systemd quote escaping. Writes `/etc/systemd/system/wakewake-agent.service` (0644) → `daemon-reload` → `enable --now`.
- Docker-route users **do not need** and **must not use** it: no systemd inside the container; `--restart unless-stopped` already covers it.

UI companion: the `/agents` advanced fold gains a "start on boot" block (the command + a "not needed for Docker installs" note), 8 locales.

## Self-signed TLS: `tls.ca_cert`

The root conflict: `danger-insecure-tls` is a compile-time feature (E2E-only; the CI release check verifies the production binary never carries it), so prebuilt binaries cannot trust Caddy `tls internal` self-signed certificates. The fix is to **trust your own CA, not to disable verification**:

- The agent gains `[tls] ca_cert = "<PEM path>"` (env `WAKEWAKE_TLS__CA_CERT`); `http_client::build_http_client` reads the PEM and applies `reqwest::Certificate::from_pem` + `add_root_certificate`. The default `None` uses the system trust store — behavior unchanged.
- LAN self-hosters export the Caddy internal CA root once (`/data/caddy/pki/authorities/local/root.crt`) to the agent machine and point config.toml at it.
- The `danger-insecure-tls` compile-time feature stays as is (E2E-only); the production binary **never** carries a "trust any certificate" switch — the anti-social-engineering boundary does not retreat.
- `build_http_client` returns `anyhow::Result` (a failed CA-file read yields a readable error; PEM validity is finally checked by reqwest/rustls at handshake time).

## The Docker route: `--network host` is a hard requirement

- **Why**: the magic packet targets the directed broadcast `255.255.255.255:9`, which routers/bridges never forward (RFC 919). Under a bridge network the broadcast floods only `172.17.0.0/16`; no packet reaches the physical NIC — and `send_to` returns Ok, the agent reports `success:true`: a **silent failure** (`wol.rs` records `source_ip` for exactly this triage).
- **Why host networking works**: the container shares the host network stack, so the `0.0.0.0` + `SO_BROADCAST` directed broadcast leaves via the default-route NIC (the LAN NIC). The agent only dials out and listens on nothing — host mode costs no security.
- The `docker run` commands in the UI and the README **must** carry `--network host` + `--restart unless-stopped` + `-v wakewake-agent-data:/data`.
- Docker Desktop (Mac/Win) host networking is incomplete — irrelevant to agent-host scenarios (NAS/Pi/Linux); the README does not elaborate.

## UI (agents page terminal card)

- The two generators in `lib/agent-command.ts`: `buildAgentInstallCommand` (the curl one-liner, primary tab) and `buildAgentDockerCommand` (the docker run one-liner); **one command, one line** (the terminal card scrolls horizontally, `$` prefixes render per line, and continuation characters would break the visuals). `--server` takes `window.location.origin`; the pairing code is passed in by the caller (reusing the template-slot mechanism; page structure untouched, only a tab switch added to the terminal card header).
- Tabs: `Linux` (default) / `Docker`; the copy button copies the current tab's command. Shared by the pending guidance state and the offline repair state.

## Acceptance

- `service.rs`: unit tests cover template rendering (fields, systemd quoting for paths with spaces), euid parsing, and the missing-config error path; `cargo test -p wakewake-agent` green.
- `install.sh`: `sh -n` syntax check + shellcheck clean; hash mismatch / unsupported platform / missing-argument paths have explicit error copy.
- CI: `release.yml` produces three-target tar.gz + .sha256; the `docker-publish.yml` agent job produces the multi-arch `jukanntenn/wakewake-agent` image; neither workflow carries `danger-insecure-tls`.
- Frontend: `agent-command.test.ts` covers both generators (origin/pairing-code embedding, `--network host` present); new i18n keys exist in all 8 locales (en/zh/ja/ko/de/fr/es/pt); `pnpm lint` + `test:run` + `format:check` green.
- Docs: the root README pair (`README.md` / `README.zh.md`) agent sections and the agent README (download/docker/service/ca_cert/platform matrix) match the real commands word for word — no invented channels.
