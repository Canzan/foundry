# Story Map — issue-status-move

## Backbone: "keep the board's columns accurate"
```
  PICK A STATUS ──────────► MOVE THE CARD ──────────► IT STICKS
  (dialog select OR drag)    relocates to column       state persisted
```

| Activity | Stories |
|----------|---------|
| Dialog path | US-01 (status control in edit dialog) |
| Gesture path | US-02 (drag-and-drop between columns) |

## Slices (2 — dialog first de-risks the card-move mechanic before DnD)

| # | Slice | Story | Learning hypothesis (fails if…) | Value |
|---|-------|-------|--------------------------------|-------|
| 01 | `slice-01-dialog-status` | US-01 | Disproves "status-in-dialog is a small extension of issue-edit-dialog reusing change_issue_state, with a server-driven card relocation" if folding state into the edit save or the OOB column-move needs machinery we lack. | Change status while editing; card moves |
| 02 | `slice-02-drag-and-drop` | US-02 | Disproves "DnD is a small, self-contained client JS over the shipped /state persist" if the chosen DnD approach (ODD-1) or the optimistic move+revert (ODD-2) is heavier/flakier than expected. | Drag a card between columns |

Slice 01 ships the **card-relocation mechanic** (server OOB or client move) that slice 02 reuses. Slice 02 adds
the app's first JS (approach = DESIGN ODD-1).

### Taste tests
- Slice 01: extends one dialog + reuses the state backend — thin. Slice 02: one JS file + wiring — thin-ish but
  novel; DESIGN de-risks the approach. Each is one end-to-end vertical; production data (real board). ✓

## Prioritization
01 then 02 (01 de-risks card-move + reuses the just-shipped dialog; 02 tackles the novel JS with the mechanic
already proven). Dogfood each on the live board.
