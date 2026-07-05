# Story Map — board-new-issue

## Backbone (user activity: "capture work from the board")

```
  OPEN ────────────► FILL ────────────► FILE ────────────► SEE IT
  click New issue    type a title        submit             card in Backlog,
                                                             modal closes
```

| Activity | Story |
|----------|-------|
| Open · Fill · File · See it | US-01 (the whole button→modal→create→card loop) |

## Walking skeleton

Already shipped server-side (modal endpoint + create POST + OOB card). This feature is the client wiring only
— no new skeleton.

## Elephant-carpaccio slices

| # | Slice | Story | ≤1 day | Learning hypothesis (fails if…) | Value |
|---|-------|-------|--------|-------------------------------|-------|
| 01 | `slice-01-new-issue-button` | US-01 | yes (~0.5 day) | Disproves "the button is pure htmx wiring over the shipped OOB create" if the modal-container/close/error-in-modal interplay needs JS beyond htmx attributes, or the no-JS fallback breaks. | Clicking "New issue" files an issue and shows the card |

### Carpaccio taste tests
- Ships 0 new components (edits 2 templates + acceptance glue) ✓.
- Depends on NO new abstraction — all shipped seams ✓.
- Disproves a real pre-commitment (htmx-only wiring, OOB close) ✓.
- Production data (real board, real create) ✓.
- Single user-visible value story — not infrastructure-only ✓.

## Prioritization

One slice; ship it. Dogfood: on the live board, click "New issue", file one, watch the card appear in Backlog.
