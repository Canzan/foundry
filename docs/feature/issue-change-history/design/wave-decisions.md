# DESIGN Decisions — issue-change-history

## Key Decisions
- [D1] **Dedicated append-only `issue_change_events` table** (migration `0013`), per-field old→new, mirroring the
  `comments` model; NOT the outbox (new-value-only, 8 KB-capped, SSE-coupled, coarse). Indexes `(issue_id,
  created_at)` + `(project_id, created_at)`. (ADR-001)
- [D2] **In-tx capture via a shared `record_issue_change` helper** at each mutation; one row per changed field.
  `update_issue_details` is restructured to a transaction (it has none today) so title/desc changes are recorded;
  status/rank capture ride `reposition_issue_with_outbox`'s existing tx. (ADR-001)
- [D3] **Start empty (ODD-5)** — v1 records field *changes* only; no backfill, no 'created' event; an unchanged
  issue shows an empty timeline. `old_value` nullable for a future creation kind. (ADR-001, upstream UC-1)
- [D4] **Human timeline → a new issue-detail page** `/team/{t}/project/{p}/issues/{n}`; card gains a link, the
  quick-edit modal is preserved. (ADR-002, upstream UC-2)
- [D5] **Program feed → `GET /api/v1/.../issues/{n}/history`** mirroring the comments route (same auth +
  non-enumerable 404 JSON); oldest→newest; reserved `cursor`. Same table as the timeline. (ADR-002)
- [D6] **Project report → a report page + CSV export** (`Content-Disposition: attachment`, mirroring
  `attachments.rs`); status-flow + per-actor summaries; `(project_id, created_at)` index. (ADR-002)
- [D7] **Append-only, immutable** — no UPDATE/DELETE code path; written in-tx with the mutation. (ADR-001)
- [D8] Repo legacy multi-file convention; no `docs/product/` SSOT; DES exempt; imperative Rust (no CLAUDE.md
  paradigm write).

## Architecture Summary
- Pattern: modular monolith, ports-and-adapters (unchanged).
- Key components touched: `record_issue_change` + `issue_change_events` + list-reads (foundry-store, migration
  `0013`); `reposition_issue_with_outbox` + `update_issue_details` capture (foundry-store); actor threading +
  list-change reads (foundry-services); the issue-detail page + project report page + card link (foundry-app web);
  `GET .../issues/{n}/history` (foundry-api).

## Reuse Analysis
| Existing Component | File | Overlap | Decision | Justification |
|--------------------|------|---------|----------|---------------|
| `comments` table + index | `0004_comments.sql` | per-issue append-only sub-record | MIRROR (new) | mirror shape + `(issue_id, created_at)` index |
| `reposition_issue_with_outbox` | `lib.rs:1364` | state+rank write, reads old_state in-tx | EXTEND | record field=status/rank via helper |
| `update_issue_state_with_outbox` | `lib.rs:1287` | state write | EXTEND or RETIRE | capture if live; delete if superseded (dead-code policy) |
| `update_issue_details` | `lib.rs:1524` | title+desc write, no tx, ignores actor | EXTEND (restructure) | tx + read-old + record title/description |
| `insert_issue_with_outbox` | `lib.rs:1169` | create | REUSE | v1 records changes, not creation (start-empty) |
| `foundry-api` routes | `foundry-api/src/lib.rs:248` | `/api/v1` issue sub-resources | EXTEND | GET history mirroring comments GET |
| board card + edit dialog | `projects.rs`, `issue_card.html`, `IssueEditModal` | issue web surface | EXTEND | card link to detail page; modal preserved |
| attachment download | `attachments.rs` | Content-Disposition | MIRROR | report CSV export |
| migration `0013`; detail page; report page; `record_issue_change` | — | — | CREATE NEW | inherently new |

## Technology Stack
- No new deps. Postgres table + 2 btree indexes; sqlx tx; Askama templates; `/api/v1` JSON + attachment-CSV
  patterns — all existing idioms.

## Constraints Established
- Append-only, immutable; in-tx capture (no phantom/drop); one row per changed field.
- One model → three surfaces (no second source of truth); field-agnostic (CHECK extensible).
- Tenancy/non-enumerability on every surface (never a 500); no outbox→SSE regression.
- `GET history` oldest→newest (audit order); human timeline newest-first (reading order).

## Upstream Changes
- UC-1 (genesis = start empty; no created event) and UC-2 (timeline home = issue-detail page) —
  `upstream-changes.md`. Both resolve open DISCUSS ODDs; no scope change.

## Handoff to DISTILL
Acceptance should cover: the in-tx record contract at each capture point (status via reposition; title/description
via the restructured update_issue_details; rank) incl. no-op = no event and multi-field = one row per field;
append-only (a second change appends, earlier unchanged); the issue-detail page rendering the timeline
newest-first + the board still opening the quick-edit modal; the `GET /api/v1/.../history` JSON (oldest→newest,
same events, non-enumerable 404); the project report + CSV (status-flow + per-actor, workspace-scoped); empty
timeline for an unchanged issue (UC-1). Verify cross-viewer convergence on reload (no live push). Confirm whether
`update_issue_state_with_outbox` is live and capture or delete accordingly.
