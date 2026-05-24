-- crates/foundry-store/migrations/0003_outbox_notify.sql
-- Slice 2 (US-09): wire outbox INSERTs into pg_notify('issue_events', ...).
--
-- Why a trigger and not direct NOTIFY in the handler:
-- - Any future code that inserts into outbox automatically fans out;
--   no per-service audit required (realtime-roadmap.md invariant #1).
-- - The trigger runs in the same transaction as the INSERT; the
--   Postgres NOTIFY queue dispatches only on COMMIT (not at trigger
--   time), so the "notify only after commit" invariant
--   (realtime-roadmap.md #3) is preserved automatically.
-- - The trigger composes the published payload from the row columns
--   (event_type + payload), injecting schema_version + timestamp so
--   slice-2 listeners get a self-describing envelope. Adding a field
--   here does NOT change the on-disk row.
--
-- Payload shape (8000-byte cap per realtime-infrastructure.md):
-- {
--   "event_type": "IssueCreated" | "IssueUpdated" | "CommentAdded",
--   "schema_version": 1,
--   "timestamp": "<ISO-8601 UTC>",
--   ...row payload fields (issue_id, project_id, workspace_id, number, key, author_id, ...)
-- }

CREATE OR REPLACE FUNCTION notify_outbox_event() RETURNS trigger AS $$
DECLARE
    envelope JSONB;
BEGIN
    envelope := jsonb_build_object(
        'event_type', NEW.event_type,
        'schema_version', 1,
        'timestamp', to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
    ) || NEW.payload;
    PERFORM pg_notify('issue_events', envelope::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER outbox_after_insert
    AFTER INSERT ON outbox
    FOR EACH ROW
    EXECUTE FUNCTION notify_outbox_event();
