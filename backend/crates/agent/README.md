# wakewake-agent

English | [中文](README.zh.md)

The WakeWake agent: an SSE client + RSA key management + WoL (magic packets) + Bemfa MQTT integration. It runs on a target machine inside the user's LAN, receives wake commands dispatched by the server, and sends the magic packet.

## Installation

### One-line install (recommended)

The server's agents page generates a one-line command with the pairing code embedded (`--server` takes your origin):

```bash
curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh \
  | sh -s -- --server https://your-wakewake.example.com --code a1b2c3d4e5f60718
```

The script: platform detection → download the release binary (musl static) → sha256 verification → install → write `~/.wakewake/config.toml` → **start in the foreground** (the page turns green when pairing succeeds). It is idempotent — after the pairing code rotates, re-run the same one-liner.

Supported platforms: `x86_64` / `aarch64` / `armv7` (Linux musl static; immune to old NAS firmware glibc). Windows / macOS are not supported yet (planned).

### Manual download

Download `wakewake-agent-<target>.tar.gz` from the [releases](https://github.com/jukanntenn/wakewake/releases), verify the `.sha256`, unpack and install; or build from source:

```bash
cd backend
cargo build --release -p wakewake-agent
# 产物：backend/target/release/wakewake-agent
```

### Persistent run (bare-metal install)

After the foreground run checks out, Ctrl+C and install it as a systemd service (starts on boot, survives ssh disconnect):

```bash
sudo wakewake-agent service install
```

It runs as the original user (`SUDO_USER` is honored under sudo) and reads the `~/.wakewake/config.toml` written at install time.

## Configuration

`server_url` and `pairing_code` are required; pick any of the three ways (they mix; the higher priority overrides the lower):

### Option 1: config file (recommended for long-term use)

Search paths (in order; later overrides earlier):

1. `$WAKEWAKE_HOME/config.toml` (default `~/.wakewake/config.toml`)
2. `./wakewake.toml` (current working directory)
3. `--config <path>` explicit

Create the config file (see [`config.example.toml`](config.example.toml)):

```toml
# ~/.wakewake/config.toml
server_url   = "https://wakewake.app"
pairing_code = "a1b2c3d4e5f60718"
```

Then just run it (no arguments needed):

```bash
wakewake-agent
```

### Option 2: environment variables

```bash
WAKEWAKE_SERVER_URL=https://wakewake.app \
WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
./wakewake-agent
```

Suited to Docker / systemd. The full field mapping lives in [`config.example.toml`](config.example.toml).

### Option 3: CLI arguments

```bash
wakewake-agent --server https://wakewake.app --pairing-code a1b2c3d4e5f60718
```

`--help` lists every argument.

## Self-hosting inside the LAN (self-signed TLS)

If the server is deployed on the LAN behind Caddy `tls internal` (self-signed certificate), point the config at your root CA certificate (**trust your own CA; do not disable verification**):

```toml
# ~/.wakewake/config.toml
[tls]
ca_cert = "/etc/wakewake/caddy-root.crt"
```

Export the root CA (inside the unified image's /data volume):

```bash
docker exec <wakewake容器> cat /data/caddy/pki/authorities/local/root.crt > caddy-root.crt
```

The prebuilt binaries and the production image ship **no** "trust any certificate" switch. `danger-insecure-tls` is a compile-time feature used only by E2E tests (E2E builds its own binary with it; it is absent from release assets).

## Docker

Inject configuration through environment variables with `WAKEWAKE_HOME=/data` (persists key.pem). **`--network host` is mandatory**: the magic packet is a directed broadcast (RFC 919 does not forward it across subnets), and under a bridge network the broadcast never leaves the container — silently (the agent reports success while the target NIC receives nothing):

```bash
docker run -d \
  --name wakewake-agent \
  --network host \
  --restart unless-stopped \
  -e WAKEWAKE_SERVER_URL=https://wakewake.app \
  -e WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
  -v wakewake-agent-data:/data \
  jukanntenn/wakewake-agent:latest
```

Image: `jukanntenn/wakewake-agent` (Docker Hub, multi-arch amd64/arm64, published per release tag). There is no systemd inside the container; `--restart unless-stopped` provides persistence, no `service install` needed.

## First connection

On first connect the agent:

1. generates an RSA-2048 keypair (persisted to `$WAKEWAKE_HOME/key.pem`, mode 0600);
2. reports the public key through the SSE connection's `X-Public-Key` header, completing pairing;
3. then receives state/command events.

The pairing code can be rotated on the server's agents page (the old code becomes invalid immediately).

## Configuration fields

The full field reference, environment variable mapping, and defaults live in [`config.example.toml`](config.example.toml).
