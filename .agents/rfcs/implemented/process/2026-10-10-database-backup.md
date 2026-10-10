# RFC: Database backup — daily pg_dump to object storage with freshness checks and restore drills

Status: implemented

English | [中文](2026-10-10-database-backup.zh.md)

## Problem

Every wakewake environment stores its entire state — users, devices, agents, integrations, wake history — in a single Postgres database inside a `pgdata` named volume on the host. Nothing else about the deployment is hard to recreate: the image is in a registry, config is templated from the repo, and Caddy is stateless. The database is the one asset whose loss is unrecoverable, and it currently has no backup at all: a host disk failure, a bad migration, an accidental `docker volume rm`, or ransomware on the VPS each mean total data loss with no rollback path.

A backup that exists but has never been restored is a hypothesis, not a capability — so the design has to make verification routine, not an act of heroics during an incident.

## Decision

Each monitored environment (test, prod) takes a daily logical dump and stores it offsite in an object-store bucket, verifies its own freshness every day, and proves restorability with a monthly drill. All of it runs as the deploy user, with no root on the host.

- **Dump pipeline** — `devops/ansible/files/pg-dump-backup.py` streams `pg_dump --no-owner` out of the compose postgres container through `zstd -19` into `rclone rcat`, landing one object per day at `dumps/wakewake-YYYYmmddTHHMM.sql.zst` in the environment's bucket. The plain-SQL dump is the format-diversity hedge: it restores on any Postgres 17 regardless of block-level state, survives base-image or page corruption that would break physical schemes, and stays human-readable in an emergency. `rclone` is rate-limited (`--bwlimit 2M`) so a dump never starves origin traffic on a thin uplink.
- **Scheduling** — systemd user units under linger (`wakewake-backup@.service` template + three timers): dump 03:30, freshness check 05:30, restore drill monthly on the 2nd at 05:00 (the 1st's dump exists to restore). `Persistent=true` catches missed runs after downtime. Logs go to the journal.
- **Freshness check** — `backup-check.py` fails when the newest offsite dump is older than 26 h (silent backup death: missed timer, full disk, revoked object-store key) or when local postgres is unreachable (every future dump will fail too), and pushes the verdict to the uptime-kuma `db backup` push monitor — so backup failure pages like any other outage instead of hiding in a log file.
- **Restore drill** — `backup-drill.py` pulls the newest dump, restores it into a throwaway `postgres:17-alpine` container, compares `users`/`wakes` row counts against the live database (wakes slack 2000 ≈ a day of writes at personal-tool scale), and removes every trace. Row-count equality, not just exit codes, is the assertion.
- **Credentials and guard** — `backup.env` (0600, rendered from `templates/backup.env.j2`) carries the vault-encrypted object-store key pair; endpoint/region/bucket are non-secret per-env vars, with test and prod in separate buckets so the promotion chain never mixes them. The whole tier is guarded on `b2_key_id` being defined in the vault: until the operator provisions the bucket and keys, the deploy stays green and byte-identical to the pre-backup pipeline (setup-order contract, activation runbook in [devops/backup.md](../../../../devops/backup.md)). Removing the vars never uninstalls anything.

## Verification

On the test environment: the dump pipeline stored a zstd-compressed object in the bucket and `rclone lsl` listed it; `backup-check.py` reported freshness and pushed an `up` verdict to its kuma monitor; `backup-drill.py` restored the newest dump into the throwaway container and matched live row counts. The playbook rendered with the tier both guarded-off and guarded-on without touching unrelated tasks.

## Alternatives considered

**WAL archival (continuous PITR).** Lost: for this dataset — a personal wake-on-LAN tool with a few tables and near-zero write rate — a 24 h RPO is worth minutes of setup, while WAL archival buys a sub-minute RPO nobody needs at a real cost: a derived postgres image with pgBackRest baked in (the stock `postgres:17-alpine` carries neither pgbackrest nor a build path the deploy user can run), `archive_command` coupling into the compose command line, spool/queue disk management on a 2 GB VPS, and a restore path (base backup + WAL replay) too intricate to verify casually. If the RPO requirement ever tightens, the daily-dump tier below remains useful as the corruption hedge and PITR can layer on top.

**Local-only dumps (host disk).** Lost: the same disk that holds the database holds the backup — disk failure, enclave compromise, and accidental deletion take both copies at once. Offsite is the entire point; the object store also gives versioning and lifecycle retention for free.

**Managed snapshots (cloud vendor).** Lost: environments span an Aliyun VPS and home LAN machines; there is no single vendor surface, snapshots are block-level (inherit page corruption), and restore requires vendor console access that the deploy path deliberately avoids.

**Cron + supervisor on the host.** Lost: both need root to install on these hosts, and the deploy playbook's contract is that the deploy user needs no passwordless sudo. systemd user units with linger give the same always-on semantics (timers fire on schedule, `Restart=always` covers the heartbeat-style jobs) entirely in user space.

## Consequences

- One zstd-compressed plain-SQL object per environment per day lands offsite; retention is bucket lifecycle policy (dumps expire after 14 days by default), documented in [devops/backup.md](../../../../devops/backup.md).
- Backup health is monitored like the service itself: a dead backup turns a kuma monitor red within a day, a broken restore path turns the monthly drill red, and neither can fail silently.
- The deploy user's home accumulates a static `rclone` binary under `~/.local/bin` on hosts lacking the package — pinned by `rclone_version` in `group_vars/all.yml`, no root involved.
- RPO is 24 h: up to a day of wake history and account changes can be lost in a total-loss scenario. The restore runbook (stop postgres → replay dump → verify → fresh full dump) lives in [devops/backup.md](../../../../devops/backup.md); drill slack is tuned to the current personal-tool write rate and would need revisiting at higher volume.
