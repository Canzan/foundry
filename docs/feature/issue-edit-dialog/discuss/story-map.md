# Story Map — issue-edit-dialog

## Backbone (user activity: "keep an issue accurate from the board")

```
  OPEN ──────────────► EDIT ──────────────► SAVE ──────────────► SEE IT
  click a card          change title/desc     Save                card updates in place,
  (dialog pre-filled)                          (persist + swap)    dialog closes
```

| Activity | Story |
|----------|-------|
| Open · Edit · Save · See it | US-01 |

## Walking skeleton

Partially shipped: the modal infra (`#modal-root` + `.modal` styling, board-new-issue) and the outbox-update
pattern (state). The NET-NEW load-bearing piece is the **issue title/description update path** (store method +
service + endpoints) — DESIGN designs it; DELIVER ships it as the skeleton of slice 01.

## Elephant-carpaccio slices (DESIGN may split further)

| # | Slice | Story | Learning hypothesis (fails if…) | Value |
|---|-------|-------|--------------------------------|-------|
| 01 | `slice-01-edit-title-description` | US-01 | Disproves "issue edit cleanly mirrors change_issue_state (store+service) + the comment inline-edit flow, reusing the board-new-issue modal + OOB-card-replace" if the save-swap (replace card in place + close dialog) or concurrency needs machinery the shipped patterns don't provide. | Click a card → edit title/description → save → card updates |

If DESIGN finds the backend + the save-swap warrant separation, it may split into: (a) the update backend +
store test, then (b) the dialog wiring + OOB-replace. DISCUSS records it as one slice; DESIGN/roadmap decide.

### Carpaccio taste tests
- New components: a store method + service method + 2 handlers + 1 view + template — this is a REAL slice
  (net-new backend), acknowledged; still one end-to-end vertical (click→save→see), ≤1 day if it mirrors the
  state/comment patterns closely.
- Reuses the board-new-issue modal + the comment-edit + state-outbox patterns — no NEW abstraction.
- Disproves a real pre-commitment (that edit mirrors the shipped update/inline-edit patterns) ✓.
- Production data (real issue, real board) ✓.
- Single user-visible value story ✓.

## Prioritization

One slice for v1 (title + description). Deferred increments (state/priority/assignee controls in the same
dialog) come after, each adding one field + its update path. Dogfood: click `GEN-1`, edit, save, watch the
card change.
