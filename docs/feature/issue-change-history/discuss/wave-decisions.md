# DISCUSS Decisions — issue-change-history

## Key Decisions
- [D1] **One durable change-event model, three surfaces.** Every tracked-field mutation writes an immutable event
  (actor · field · old → new · when); the human timeline, the program JSON feed, and the project report all read
  the SAME events. No per-surface source of truth.
- [D2] **In-transaction capture, append-only.** The event is written in the same tx as the mutation (no phantom /
  no drop); entries are never edited or deleted (audit integrity). No retention/pruning in v1.
- [D3] **Four slices, model first** (user-confirmed all three surfaces in v1): slice 01 status-timeline (the model
  + capture + human surface — walking skeleton), 02 all-fields (widen capture, incl. the silent title/desc path),
  03 program JSON feed, 04 project report + CSV. Each slice reuses the slice-01 model.
- [D4] **Records changes; does NOT add editing.** Tracked fields = those editable today (status, title,
  description, rank). Priority/assignee editing is a SEPARATE feature; the field-agnostic model absorbs them for
  free once they become editable — so v1 records nothing for them (nothing changes them yet).
- [D5] **Reuse the durable outbox as PRECEDENT / candidate source.** The outbox (`0003`) is already append-only
  and durable, but new-value-only + notify-shaped + 8 KB-capped — DESIGN decides enrich-and-reuse vs a dedicated
  per-field `issue_events` table (ODD-1). Either way the realtime outbox→SSE path stays green.
- [D6] **Real DESIGN required.** The change-event model, the store choice, the in-tx capture across four write
  paths, the timeline's home, the report shape, and the program envelope are genuine architecture decisions.
- [D7] **Repo legacy multi-file convention; no SSOT (`docs/product/` absent); DES exempt.** JTBD captured inline
  (no `jobs.yaml` bootstrap), matching all prior features on trunk.

## Requirements Summary
- Primary need: make every change to an issue a durable, attributable record — readable by a human, consumable by
  a program, aggregable by a report — from one model, with nothing silently lost.
- Walking skeleton: slice 01 — the change-event model + in-tx capture at the status write path + the human
  timeline, end-to-end.
- Feature type: cross-cutting (durable persistence + web render + `/api/v1` + reporting).

## Constraints Established
- In-tx capture (no phantom/drop); append-only immutable; no retention v1.
- One model → three surfaces (no second source of truth); field-agnostic + extensible.
- Tenancy/CSRF; foreign/absent issue → uniform non-enumerable refusal on every surface (never a 500).
- No regression to the shipped outbox → SSE realtime path.

## Scope Assessment: PASS
Right-sized as 4 thin slices (≤4 stories, one migration, reuses tenancy/API/render patterns). Slice 01 carries the
only deep novelty (the one model serving all three surfaces); 02–04 are read/capture extensions over it.

## Handoff to DESIGN
Resolve ODD-1 (history store: enrich+reuse outbox vs a dedicated `issue_events`/`issue_history` table),
ODD-2 (in-tx capture mechanism across the four write paths incl. the silent `update_issue_details`; one row per
changed field), ODD-3 (human-timeline home: edit dialog vs a new issue-detail page), ODD-4 (report shape +
dimensions + CSV), ODD-5 (genesis entry for pre-existing issues), ODD-6 (program JSON envelope + pagination
readiness over `/api/v1`). Plus: the exact tracked-field set + the plain-language phrasing map for the human view.

## Upstream Changes
None — this is a new capability grounded in the shipped issue write paths, the durable outbox (`0003`), the
comments precedent (`0004`), and the `/api/v1` router. Brownfield; no prior-wave assumptions changed.
