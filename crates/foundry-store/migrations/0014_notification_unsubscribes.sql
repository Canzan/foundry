-- crates/foundry-store/migrations/0014_notification_unsubscribes.sql
-- recipient-notification-preferences (ADR-004) — per-(recipient-email, workspace)
-- opt-out for the two SUPPRESSIBLE notification events (workspace_invite,
-- member_invite). Default state = NO ROW = subscribed (opt-out model, BR-7);
-- presence of a row = muted. `email_lower` matches users.email_lower /
-- find_user_by_email(email_lower) — there is deliberately NO FK to users because
-- many recipients are account-less invitees (BR-2). The composite PRIMARY KEY IS
-- the covering index for the suppression point-read (WHERE email_lower=$1 AND
-- workspace_id=$2) and enforces idempotence (INSERT ... ON CONFLICT DO NOTHING).
CREATE TABLE notification_unsubscribes (
    email_lower     TEXT        NOT NULL,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    unsubscribed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (email_lower, workspace_id)
);
