# ADR-007: Comment Soft-Delete with Tombstone

## Status
Accepted — 2026-05-25

## Context

Slice 5 implements the US-10 AC for comment delete, which has two actors:

- **Author delete**: a member deletes their own comment.
- **Admin delete**: a workspace admin deletes any comment as a moderation
  action.

The admin-delete case raises an auditability requirement: if Mei reports
"Devansh deleted my comment unjustly", the system must be able to
demonstrate (at least to operators) that the deletion happened, by whom,
and when. The slice-3 backup/restore story established a high bar for
audit posture; comment deletion needs to fit within it.

Three families of delete policy exist:

- **Hard DELETE** — `DELETE FROM comments WHERE id = $1`. Row is gone
  from the database; gone from backups; gone from any future audit
  request.
- **Soft (tombstone)** — add `deleted_at` / `deleted_by` columns; UI
  hides rows where `deleted_at IS NOT NULL`; `pg_dump` captures
  everything; undelete is a single `UPDATE`.
- **Hybrid (soft now, GC later)** — soft-delete on the write path; a
  background task (using the slice-3 advisory-lock cleanup pattern)
  hard-deletes rows where `deleted_at < now() - interval '90 days'`. The
  audit window is bounded; storage stays bounded; GDPR-friendly.

Quality attributes driving this decision: **auditability (HIGH)** — the
US-10 admin-delete AC requires it; **recoverability (MEDIUM)** —
accidental deletes can be reversed; **privacy (MEDIUM)** — bounded
retention for deleted content; **simplicity (HIGH)** — slice 5 is a
2-day slice and shouldn't accumulate a 5th cleanup task.

## Decision

**Soft-delete with tombstone.** Slice 5 adds three nullable columns to
the `comments` table via migration `0006_comments_edit_delete.sql`:

- `updated_at TIMESTAMPTZ NULL` — supports the "edited" indicator (Q4 = A).
- `deleted_at TIMESTAMPTZ NULL` — tombstone marker.
- `deleted_by UUID NULL REFERENCES users(id)` — admin actor for audit.

Every read path against `comments` (the list query, the
`find_comment_by_id` accessor) MUST be aware of tombstone semantics:

- The public list query filters `WHERE deleted_at IS NULL`.
- `find_comment_by_id` returns tombstones too, so the handler can
  distinguish 404 (no row) from 410 (tombstoned row) per ADR-008.
- The moderation audit path queries tombstones explicitly (no UI in
  slice 5; operator-only via `psql`).

The 90-day GC task (alternative C below) is explicitly DEFERRED to a
v0.2 follow-up. The slice-5 schema is a **strict subset** of the schema
that GC needs; no further migration is required when v0.2 ships GC.

## Alternatives Considered

### A: Hard DELETE
`DELETE FROM comments WHERE id = $1`. Outbox carries
`CommentDeleted {comment_id}` for fanout.

- **Pros**: Smallest schema delta (no new columns). Backup/restore is
  symmetric — deleted comment is not in `pg_dump`, period. No risk of
  "deleted but visible" bugs (the row is gone).
- **Cons**: Lost-forever moderation history. Admin auditability
  impossible at the data layer. If Mei reports "Devansh deleted my
  comment unjustly", there's no row to point at. SSE event must be
  self-describing (no follow-up "fetch the row" path possible).
- **Rejected because**: fails the audit posture established by slice 3.
  Saves a one-migration cost at the price of a permanent moderation
  blind spot.

### B: Soft (tombstone) — CHOSEN
See Decision.

- **Pros**: Full moderation audit trail. `pg_dump` captures everything
  (slice-3 backup story holds). Undelete is `UPDATE ... SET deleted_at
  = NULL`. SSE event can carry `{deleted: true, comment_id}` and the
  receiver re-renders the comment card with tombstone styling.
- **Cons**: Storage grows monotonically (until GC arrives). Privacy
  concern: deleted comment body lives forever in backups (matters for
  GDPR-ish workspaces). UI logic must consistently apply the
  `WHERE deleted_at IS NULL` filter (one missed `WHERE` and deleted
  comments leak — mitigated by acceptance-suite enforcement; v0.2 may
  introduce a `comments_visible` SQL VIEW to make it schema-level).

### C: Hybrid — soft now, GC at 90 days
Soft-delete on the write path (B above). A background task in
`foundry-app` (Postgres advisory lock for single-replica execution, per
the existing cleanup-task pattern from slice 1) hard-deletes rows where
`deleted_at < now() - interval '90 days'`.

- **Pros**: Audit trail for the 90-day window when moderation disputes
  happen. Storage stays bounded. GDPR-friendly — privacy stewards can
  document "deleted comments are unrecoverable after 90 days".
- **Cons**: One more background task (cron-style cleanup) — a 5th
  cleanup pass alongside expired sessions, expired bootstrap tokens,
  expired reset tokens, expired invites. The 90-day knob becomes a
  config value or a hardcoded constant.
- **Deferred to v0.2 follow-up**: B + C share the same schema (B is a
  strict subset of C). Shipping B now with C as a follow-up costs
  nothing — no schema migration is required to upgrade. The follow-up
  is purely a new background task + config knob.

## Consequences

### Positive
- Admin moderation actions are auditable. `deleted_by` records who; the
  row body stays intact for "what was deleted" review.
- Undelete is trivial — a single `UPDATE comments SET deleted_at = NULL,
  deleted_by = NULL WHERE id = $1`. Operator runbook addition is a
  one-line `psql` recipe (intentionally not exposed as a UI in slice 5).
- Slice-3 backup/restore story continues to work — tombstones ride along.
- SSE event shape is clean — receiver carries `{deleted: true}` and
  re-renders the comment card with tombstone styling.

### Negative
- Storage grows monotonically until v0.2 ships GC. Negligible for
  expected slice-5 deployment scale (a 1000-comment instance with 5%
  deletion is ~50 rows of ~few-hundred bytes each).
- GDPR-like "right to be forgotten" requests are not satisfiable in
  slice 5 (the body persists). Operators with strict privacy
  requirements must wait for v0.2 GC or run a manual cleanup SQL until
  then. Documented in the slice-5 operator runbook.
- The "must always filter `WHERE deleted_at IS NULL`" rule is convention
  rather than schema enforcement in slice 5. Acceptance suite covers
  the public list path; future read paths need the same coverage.

### Neutral
- The B-to-C upgrade requires zero schema migration. New cleanup task +
  one configuration knob.
- `0006_comments_edit_delete.sql` bundles the `updated_at` column
  (driven by Q1+Q4) alongside the `deleted_at` / `deleted_by` columns.
  One migration, one transactional schema bump.

## Verification

- Slice-5 acceptance scenario: admin deletes a comment; row still
  exists with `deleted_at IS NOT NULL`; public list query returns zero
  results for that issue; operator `SELECT * FROM comments WHERE
  deleted_at IS NOT NULL` returns the row with `deleted_by` populated.
- The probe assertion (Earned Trust, principle 12) on the existing
  `Store::probe()` confirms migration `0006` applied — `deleted_at`
  and `updated_at` columns exist.
- 410-Gone handler path (ADR-008 / Q6 = B) exercised: PATCH or DELETE
  on a soft-deleted row returns 410, not 404.
