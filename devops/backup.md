# Database backup (test / prod)

English | [中文](backup.zh.md)

Daily logical dumps to an offsite object-store bucket, a freshness check that pages when backups die silently, and a monthly restore drill that proves the dumps restore. Rationale and rejected alternatives: [backup RFC](../.agents/rfcs/implemented/process/2026-10-10-database-backup.md).

## What runs where

| Piece | Where | Schedule |
|---|---|---|
| `pg-dump-backup.py` | `{{ app_path }}` on each host, systemd user unit `wakewake-backup@pg-dump-backup.py.service` | daily 03:30 |
| `backup_check.py` | same | daily 05:30, pushes verdict to kuma |
| `backup-drill.py` | same | monthly, day 2 at 05:00 |

All three run as the deploy user (linger enabled; no root). Credentials live in `{{ app_path }}/backup.env` (mode 0600): endpoint/region/bucket from `group_vars/<env>/env.yml`, the key pair from vault (`b2_key_id` / `b2_secret_access_key`). `zstd` comes from the host package; `rclone` installs as a static binary under `~/.local/bin` when the host package is absent (version pinned by `rclone_version` in `group_vars/all.yml`).

Logs: `journalctl --user -u wakewake-backup@pg-dump-backup.py.service` (and the other two units).

## Object-store provisioning (once)

1. Create a **private, versioned** bucket: `wakewake-backups` (prod) and `wakewake-backups-test` (test) — separate buckets so promotion never mixes them.
2. Create an application key restricted to that bucket with **writeFiles, readFiles, listFiles** only — no delete. Both scripts only ever add objects.
3. Lifecycle: expire `dumps/*` noncurrent versions after 14 days (the live objects are governed by retention-full below).

## First activation (setup order matters)

The whole tier is guarded on `b2_key_id` being present in the vault — until then deploys skip it entirely and stay green.

1. Provision the bucket and key as above.
2. Vault the key pair (values never land in a plaintext file):
   ```bash
   ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client --encrypt-vault-id wakewake-test '<keyID>' --name b2_key_id >> devops/ansible/group_vars/test/vault.yml
   ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client --encrypt-vault-id wakewake-test '<appKey>' --name b2_secret_access_key >> devops/ansible/group_vars/test/vault.yml
   ```
   (same for `wakewake-prod` and the prod vault; when the kuma backup monitor exists, add `kuma_backup_url` the same way)
3. Deploy: `ansible-playbook devops/ansible/deploy.yml -l test` — scripts, `backup.env`, units, and timers land and the timers arm.
4. First dump by hand and watch it land:
   ```bash
   systemctl --user start wakewake-backup@pg-dump-backup.py.service
   journalctl --user -u wakewake-backup@pg-dump-backup.py.service -n 5
   ~/.local/bin/rclone lsl :s3,provider=Other,endpoint=<endpoint>:<bucket>/dumps/  # needs the env vars from backup.env
   ```
5. Same for the check and the drill:
   ```bash
   systemctl --user start wakewake-backup@backup_check.py.service
   systemctl --user start wakewake-backup@backup-drill.py.service   # prints live vs restored row counts
   ```

## Disaster recovery

When a host is lost and a new one is being brought up (or the database is corrupted in place):

1. Provision/redeploy the environment as usual (`ansible-playbook devops/ansible/deploy.yml -l prod -K`), but **stop before** the app writes traffic; then stop postgres:
   ```bash
   cd ~/docker/wakewake && docker compose stop wakewake
   ```
2. Fetch the newest dump and restore it into the fresh postgres container:
   ```bash
   source ./backup.env   # or parse it; then:
   ~/.local/bin/rclone cat ":s3,provider=Other,endpoint=$B2_S3_ENDPOINT:$B2_BUCKET/dumps/<newest>.sql.zst" \
     --s3-access-key-id "$B2_ACCESS_KEY_ID" --s3-secret-access-key "$B2_SECRET_ACCESS_KEY" \
     --s3-region "$B2_S3_REGION" --s3-no-check-bucket \
     | zstd -dc | docker compose exec -T postgres psql -U wakewake -d wakewake
   ```
   (if the target database is not empty — a corrupted-in-place recovery — drop and recreate it first)
3. Bring the app back and verify: `docker compose up -d`, then `curl /api/v1/health` and log in.
4. Take a **fresh full dump immediately** (`systemctl --user start wakewake-backup@pg-dump-backup.py.service`): the DR copy is now the only copy.

Point-in-time beyond the last dump is not recoverable — RPO is 24 h by design (the [RFC](../.agents/rfcs/implemented/process/2026-10-10-database-backup.md) records the trade).

## Failure modes

| Symptom | First check |
|---|---|
| kuma `db backup` monitor down | `journalctl --user -u wakewake-backup@backup_check.py.service` — the `msg` names the failing probe |
| freshness probe fails | did the dump timer fire? `systemctl --user list-timers`; disk full? object-store key expired? |
| dump pipeline exits 1 | journal shows which stage (pg_dump / zstd / rclone) failed; network vs credentials |
| drill fails on row counts | check dump age — more than a day of writes on a busy day means the slack is too tight; adjust `WAKES_SLACK` |
