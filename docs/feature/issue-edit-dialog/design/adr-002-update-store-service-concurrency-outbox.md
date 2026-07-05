# ADR-002 — Update store/service, concurrency, outbox (ODD-3, ODD-4)

**Status**: ACCEPTED (user-ratified 2026-07-05)

## Context
Only `state` is editable today (`update_issue_state_with_outbox` + `change_issue_state`). Title/description
need an update path. Two policy questions: concurrency and realtime.

## Decision
- **Store `update_issue_details(prefix, number, title, description_md, actor_id)`** — mirrors
  `update_issue_state_with_outbox` MINUS the outbox emit: lookup issue by `key_prefix+number`, `UPDATE title,
  description_md, updated_at=now()`. `Ok(None)` when absent.
- **Service `edit_issue_details(...)`** — mirrors `change_issue_state`: `resolve_member_project` authz →
  validate title (non-empty, ≤256) → `update_issue_details`.
- **ODD-3 Concurrency = last-write-wins** for v1. No `updated_at` optimistic guard; conflict UX is out of scope.
- **ODD-4 No outbox/realtime emit** in v1. The editing user sees the OOB card update immediately; broadcasting
  edits to OTHER board viewers via SSE is a later realtime increment (avoids surprising the SSE consumer with
  a new event kind now).

## Alternatives rejected
- Reuse `update_issue_state_with_outbox`-style outbox emit now: adds a realtime event the SSE consumer isn't
  built to render → risk; deferred.
- Optimistic concurrency (`updated_at` guard): valuable but v1 has no conflict UX; deferred as hardening.

## Consequences
Minimal net-new backend, no migration. Realtime edit-broadcast + concurrency guard are named deferred
increments. Title validation bounds match create (1–256).
