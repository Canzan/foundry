# ADR-BOARD-LANE-001: `issues.state` stays the lane slug, referenced by a composite FK to per-project `lanes`

## Status

Accepted (board-lane-management DESIGN wave, 2026-08-22)

## Context

The board's lane set is expressed as compile-time constants: `DEFAULT_COLUMNS`
(`foundry-app/src/projects.rs:49`), the `issues.state` CHECK
(`0001_init.sql:71-72`), `normalize_state` (`foundry-services/src/issues.rs:60`),
the edit-dialog `<option>` list, and `humanize_state`. The feature (per-project
lane sets, three-lane defaults, lane deletion — D2/D4/D7) is unimplementable
against constants. Meanwhile every *other* surface already treats the lane as an
opaque string: 0012 partitions card positions by `(project_id, state)`, 0013
`status` change events store state slugs, `data-column` attributes / the dnd POST
body / the `/api/v1` `state` field all carry the slug. Quality priorities:
correctness and testability over everything; homelab scale. Hard requirements:
zero issues may ever be laneless (KPI 2 guardrail, "provable by query"), the
grandfather migration must be zero-surprise (D5), and the two-fate lane delete
must never strand a concurrently filed card (US-BLM-04 scenario 5).

## Decision

Create a per-project `lanes` table (`id, project_id, workspace_id, slug, label,
position`, `UNIQUE (project_id, slug)`, `UNIQUE (project_id, position)
DEFERRABLE INITIALLY IMMEDIATE`). Keep `issues.state` as the lane slug. In
migration 0015: drop the static CHECK, drop `DEFAULT 'backlog'`, and add

```sql
FOREIGN KEY (project_id, state) REFERENCES lanes (project_id, slug)
```

Lane slugs carry the five existing state values 1:1 and are immutable identity;
`label` is the mutable display value (the names-are-labels invariant extends to
lanes). Validation moves from `normalize_state`'s static set to a per-project
membership check behind one shared seam (`validate_project_lane`, DD10).

## Alternatives Considered

- **A. `issues.lane_id UUID REFERENCES lanes(id)`, retire `state`** — Rejected.
  Fractures every slug-opaque surface: 0012's `(project_id, state, position)`
  partition, 0013 event values, `data-column`/dnd body, the API wire field, and
  every `WHERE state = $n` query — or forces a permanent slug↔id mapping layer.
  2–3× the code delta of the chosen option for zero user-visible benefit.
- **B. Keep the CHECK, add a per-project "enabled lanes" list validated in the
  app only (no FK)** — Rejected. One constraint simpler, but the "zero laneless
  issues" invariant becomes an application convention: any future write path can
  silently violate it, and the delete-time race guard (see
  ADR-BOARD-LANE-002) disappears. The invariant must be a schema fact.
- **C. Widen the CHECK to all five states and store lane visibility as project
  config (JSONB)** — Rejected. A JSONB blob cannot be an FK referent; deleting
  a lane could not structurally guarantee card fate; ordering and per-lane rows
  would be reinvented in application code.

## Consequences

- Positive: the five static expressions become data; every slug-opaque surface
  is byte-untouched (dnd JS, keyboard nav, API wire, CSV columns, 0012/0013
  semantics). "Zero laneless issues" is enforced by Postgres, not by tests. The
  FK's `NO ACTION` blocks lane deletion while cards reference it — the
  strand-guard the two-fate transaction leans on. Add/rename/reorder (D9
  successors) slot in without schema change.
- Negative: every state write costs one extra indexed `lanes` read (negligible
  at homelab scale). The composite FK ties `issues.project_id` and `state`
  together — a future "move issue across projects" feature must move the lane
  membership in the same statement (documented, acceptable). Historical slugs
  in 0013 events may reference deleted lanes; report labels fall back to the
  fixed `humanize_state` map for them (closed slug set, D9 — the fallback is
  total).
- Enforcement: `cargo xtask check-arch` gains a rule failing the build if a
  static lane-slug list reappears under `crates/foundry-app/src` or
  `crates/foundry-api/src` (exemptions: store creation seed, `humanize_state`
  historical fallback).
