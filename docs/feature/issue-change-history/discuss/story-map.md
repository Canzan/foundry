# Story Map — issue-change-history

## Backbone: "every change is a durable, attributable record — read it, consume it, report it"
```
  A CHANGE HAPPENS ─────► IT IS RECORDED ─────► SURFACED THREE WAYS
  (status/title/desc/rank) (actor·field·old→new    human timeline · program JSON · project report
                            ·when, in-tx, append-only)
```

| Activity | Stories |
|----------|---------|
| Record + read (human) | US-01 (status timeline) · US-02 (all fields) |
| Consume (program) | US-03 (JSON feed) |
| Aggregate (report) | US-04 (project report + CSV) |

## Walking skeleton
US-01 is the walking skeleton: the durable change-event **model** + capture at ONE write path (status) + ONE
surface (the human timeline) end-to-end. It carries the feature's core uncertainty (the model that must serve all
three surfaces). Everything else reuses that model.

## Slices (4 — model first, then breadth, then each surface)

| # | Slice | Story | Learning hypothesis (fails if…) | Value |
|---|-------|-------|--------------------------------|-------|
| 01 | `slice-01-status-timeline` | US-01 | Disproves "a durable per-field change-event model (actor·field·old→new·when), captured in-tx at the state write path, rendered as a plain-language timeline, is a clean increment" if the model, the in-tx capture, the genesis story (ODD-5), or the timeline's home (ODD-3) needs machinery we lack. | See who moved an issue's status, and when |
| 02 | `slice-02-all-fields` | US-02 | Disproves "the remaining write paths emit the SAME change-event uniformly" if the currently-silent `update_issue_details` (title/desc) or the reposition (rank) path resists a uniform in-tx record, or multi-field saves don't decompose cleanly. | The timeline is the whole field-level story |
| 03 | `slice-03-program-feed` | US-03 | Disproves "the stored change-event serializes cleanly for program consumption over the `/api/v1` contract" if the JSON envelope / auth / non-enumerability needs a second model or path. | A program can sync/react to issue changes |
| 04 | `slice-04-project-report` | US-04 | Disproves "the same events aggregate into a useful, exportable project report" if status-flow / per-actor rollups or CSV export need a separate store or a different grain. | A lead accounts for work across the project |

### Taste tests
- Slice 01 carries the only deep novelty (the change-event model + in-tx capture + the timeline's home) — DESIGN
  de-risks it; one migration + one capture point + one render, on production data (a real issue's status moves). ✓
- Slices 02–04 each REUSE the slice-01 model (no new abstraction): 02 = more capture points, 03 = a read endpoint,
  04 = an aggregate read + export. Each is one thin end-to-end vertical. ✓
- Each slice disproves a distinct real bet (model / uniform capture / program serialization / aggregation). Not
  decoration. ✓
- No slice ships 4+ new components; the shared abstraction (the model) ships FIRST as slice 01. ✓

## Prioritization
01 → 02 → 03 → 04. Slice 01 first — highest uncertainty (the one model that must serve all three surfaces); a
wrong model bet is caught cheapest there. Then 02 widens capture; 03 and 04 are independent read surfaces over the
proven model (03 before 04 — the program feed is the simpler read; the report aggregates on top). Dogfood each on
the live sandbox board (the GEN-* issues).

## Scope note
Right-sized as 4 thin slices (not oversized: ≤4 stories, one migration, reuses tenancy/API/render patterns). This
feature RECORDS changes; it does NOT add new editing surfaces — priority/assignee editing is a separate feature,
and the field-agnostic model absorbs them for free when they arrive.
