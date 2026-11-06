# ADR-IIS-005: Instance-level integration activity has its own append-only table

## Status

Proposed (instance-integration-setup DESIGN, 2026-11-05). Option 5-A was chosen by the user on
2026-11-05 (`docs/feature/instance-integration-setup/design/options.md`).

## Context

- D12: every key and Slack app change is recorded with the actor, the date and the client ID,
  never a value. It is shown as "Activity" (I-18) on the instance page and is not copied into
  workspace activity.
- `integration_audit_events` (migration 0030) has `workspace_id NOT NULL` and a closed `action`
  CHECK.

## Decision

1. **The table.** `instance_integration_audit_events` has these columns:
   - `id`;
   - `actor_id` (nullable, `ON DELETE SET NULL`; NULL means "Foundry", the system line);
   - `action`: a closed CHECK of `secrets_key_generated`, `credentials_moved`,
     `secrets_key_rewrapped`, `slack_app_saved`, `secrets_key_replaced`, `started_over`,
     `slack_app_replaced`, `slack_app_removed`;
   - `client_id` (nullable);
   - `count` (nullable);
   - `created_at`.

   It never holds a key, a secret, a key id or a length.
2. **Writes.** Each line is written in the same transaction as its change (ADR-IIS-004).
3. **Reads.** The instance page reads the latest lines, newest first. Retention is none, because
   the table is tiny.
4. **Start over (B)** also writes one existing `connection_disconnected` audit line per affected
   workspace, so that workspace's own activity explains the reconnect.

## Alternatives considered

- **Extend `integration_audit_events`** (5-B). Rejected: it alters a shipped table's NOT NULL and
  CHECK, and every workspace query must keep excluding NULL rows.
- **A general instance-admin audit log** (5-C). Rejected: wider than the feature. Other instance
  actions already have their own event tables.

## Consequences

- Positive: no change to shipped tables, and the action list maps one-to-one onto I-18.
- Negative: one more table. Acceptance greps it for planted secrets (KPI-IIS-G1).
