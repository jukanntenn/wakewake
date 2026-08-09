-- device-sync-v3 down（逆向回旧 schema，specs/backend/device-sync-v3.md §7.1）。

ALTER TABLE agents   DROP COLUMN projection_version;

ALTER TABLE devices  DROP COLUMN bemfa_observed_name;
ALTER TABLE devices  DROP COLUMN bemfa_observed_at;
ALTER TABLE devices  DROP COLUMN last_drift_at;
ALTER TABLE devices  DROP COLUMN last_drift_kind;
ALTER TABLE devices  DROP COLUMN last_error;
ALTER TABLE devices  ADD COLUMN sync_status VARCHAR(20) NOT NULL DEFAULT 'syncing';
ALTER TABLE devices  ADD COLUMN bemfa_status VARCHAR(20);
ALTER TABLE devices  ADD COLUMN bemfa_last_error TEXT;

ALTER TABLE integrations DROP COLUMN mqtt_connected;
ALTER TABLE integrations DROP COLUMN last_report_at;
ALTER TABLE integrations ADD COLUMN sync_status VARCHAR(20) NOT NULL DEFAULT 'syncing';
ALTER TABLE integrations ADD COLUMN last_synced_at TIMESTAMPTZ;
