-- Stable, unique IDs for audit events and outbound email records.
ALTER TABLE audit_logs ADD COLUMN event_id TEXT;
-- Historical rows need deterministic IDs. The append-only trigger intentionally
-- blocks normal updates, so pause that trigger only while backfilling this column.
ALTER TABLE audit_logs DISABLE TRIGGER audit_logs_no_update;
UPDATE audit_logs
SET event_id = 'evt_' || md5(id::text || ':' || created_at::text)
WHERE event_id IS NULL;
ALTER TABLE audit_logs ENABLE TRIGGER audit_logs_no_update;
ALTER TABLE audit_logs ALTER COLUMN event_id SET NOT NULL;
CREATE UNIQUE INDEX idx_audit_event_id ON audit_logs(event_id);

ALTER TABLE email_logs ADD COLUMN event_id TEXT;
UPDATE email_logs
SET event_id = 'evt_' || md5(id::text || ':' || created_at::text)
WHERE event_id IS NULL;
ALTER TABLE email_logs ALTER COLUMN event_id SET NOT NULL;
CREATE UNIQUE INDEX idx_email_event_id ON email_logs(event_id);
