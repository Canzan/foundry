# ADR-001 — Dedicated `issue_change_events` model + in-tx capture

**Status**: Accepted (user-ratified 2026-07-07) · **Feature**: issue-change-history · **Slices**: 01 + 02

## Context

Every tracked-field mutation must become a durable, immutable, attributable record serving three surfaces. A
durable outbox (`0003`) already records issue events, but new-value-only, notify-shaped, 8 KB-capped, coarse
(`IssueUpdated`, not field-level), and mixed with comment events.

## Decision

**A dedicated, append-only `issue_change_events` table (migration `0013`) modeling per-field old→new, mirroring
the `comments` precedent; changes are captured in the SAME transaction as the mutation via a shared store helper.**

- **Schema** (see `architecture.md`): `id, workspace_id, project_id, issue_id, actor_id, field, old_value,
  new_value, created_at`; indexes `(issue_id, created_at)` (timeline + API) and `(project_id, created_at)`
  (report). `field` is a CHECK over the v1 tracked set (`status, title, description, rank`), extended per new
  editable field. `old_value` is nullable (reserved for a future creation-event kind); v1 field-change rows carry
  both old + new.
- **In-tx capture** — a helper `record_issue_change(&mut tx, workspace_id, project_id, issue_id, actor_id, field,
  old, new)` INSERTs within the mutation's transaction, so a rolled-back mutation records nothing and a committed
  one always records (no phantom, no drop). Capture points:
  - `reposition_issue_with_outbox` (`lib.rs:1364`) — already reads `old_state` in-tx → record `field=status`
    (slice 01) and `field=rank` on a position change (slice 02).
  - `update_issue_details` (`lib.rs:1524`) — **restructured** from its current tx-less two-statement form into one
    transaction that reads the old title/description, UPDATEs, and records `field=title` / `field=description`
    for each that actually changed (slice 02). Its ignored `_actor_id` becomes used.
  - one row **per changed field**; a same-value save records nothing (a no-op is not history).
- **Append-only / immutable** — no code path UPDATEs or DELETEs a row (audit integrity); `ON DELETE CASCADE`
  only removes history when its issue/project/workspace is itself deleted.
- **Genesis = start empty (ODD-5)** — no backfill; creation is not recorded in v1. Pre-existing and brand-new
  issues show an empty timeline until their first field change.

## Why (vs alternatives)

| Option | Diffs | Query | Coupling | Verdict |
|--------|-------|-------|----------|---------|
| **Dedicated `issue_change_events`** | per-field old→new | one indexed query per surface | decoupled | **Chosen** — clean model for all three surfaces; mirrors a proven table |
| Enrich + reuse the outbox | would need old-values added | filter jsonb payloads by issue_id; mixed with comments | every insert fires SSE; 8 KB cap; backfill spams notify | Rejected — notify-shaped + capped + coarse; awkward to query and to bulk-write |

The outbox stays exactly as-is (its realtime job is unaffected); history is a separate, purpose-built store.

## Consequences

- One new table + one shared helper; `update_issue_details` gains a transaction (a small, correctness-improving
  restructure — it also fixes that title/desc edits were silently unrecorded).
- The three read surfaces (ADR-002) all read this one table — no second source of truth.
- Concurrency: inserts are independent appends (no contention); the mutation's own tx serializes with it.
- Extensibility: priority/assignee capture is a CHECK addition + a helper call when those become editable — no
  model change.
