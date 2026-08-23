# Slice 01 — Lanes become the project's own data (walking skeleton)

Story: US-BLM-01 | Estimate: 1 day | job_id: `job-board-lane-shaping`

## Goal

Replace the hardcoded lane set (`DEFAULT_COLUMNS`, `projects.rs:49` + the
`issues.state` CHECK as UI source) with per-project lane data, end-to-end:
migration → board render → edit-dialog options → dnd targets → `/api/v1`
validation. Existing boards render byte-identically; the one visible outcome
is that stranded `cancelled` issues surface in a Cancelled lane (D1b/D5).

## IN

- Migration 0015 (forward-only): lane data per project — Backlog, Todo,
  In-Progress, Done for every existing project, plus Cancelled only where the
  project holds ≥1 cancelled issue (D5, zero-surprise like 0012's backfill).
  DB CHECK relaxation on `issues.state` is the DESIGN-owned consequence (D2).
- Board render: columns from the project's lane list (labels, slugs, order) —
  `build_board_page` stops consuming a const.
- Edit dialog: Status `<select>` options from the same lane list, board order.
- Per-project state validation replacing `normalize_state`'s static acceptance
  set on every write path — dialog save, dnd POST, `/api/v1` PATCH. Unknown
  lane → 422 validation error (D8). One seam shared by both adapters (DD10).
- Regression guard: dnd drop/position semantics (0012), `status` change events
  (0013), keyboard board-nav over `[data-column]` — all unchanged behavior,
  now against data-driven lanes.

## OUT

- New-project default seeding change (slice 02 — existing 4-lane seeding
  behavior may persist for the duration of this slice).
- Any delete affordance or lane mutation (slices 03–04).
- Add/rename/reorder lanes (feature OUT, D9).

## Learning Hypothesis

The lane set has exactly the six consumer surfaces D8 enumerates. Scenario 1
(byte-identical render) plus the existing acceptance suite either proves the
swap is invisible or flushes out an unlisted consumer of the static lane list.

## Acceptance Criteria

- [ ] "Identity Platform" (no cancelled issues): columns Backlog, Todo,
      In-Progress, Done, every card in the same column at the same position
      as before the upgrade.
- [ ] "Homelab Ops" (OPS-9 in cancelled): a Cancelled column renders after
      Done holding OPS-9; OPS-9's edit dialog offers Cancelled.
- [ ] PATCH AUTH-7 → "cancelled" on a project with no Cancelled lane: 422,
      issue unchanged; zero issues in a laneless state, provable by query.
- [ ] Dnd drag of AUTH-12 Todo → top of In-Progress persists position on
      reload and writes one `status` change event (existing behavior pinned).

## Dependencies

None (skeleton). Known suite impact: `UNRENDERED_STATE = "cancelled"` premise
in `keyboard_shortcut_bindings.rs:3891` breaks by design — re-premise or
retire that edge in DELIVER (Inherited commitments).
