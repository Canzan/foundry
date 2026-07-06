# Story Map — card-ranking-within-status

## Backbone: "the column reads in the order I plan to work it"
```
  DRAG A CARD ──────────► DROP IT AT A SLOT ──────────► THAT ORDER STICKS
  (within or across        exact position,               persisted rank,
   a status)               not appended to end           every viewer sees it
```

| Activity | Stories |
|----------|---------|
| Order within a status | US-01 (within-status reorder) |
| Triage into a status + slot | US-02 (cross-status positional drop) |

## Slices (2 — within-status first ships the whole rank machinery, then cross-status extends the gesture)

| # | Slice | Story | Learning hypothesis (fails if…) | Value |
|---|-------|-------|--------------------------------|-------|
| 01 | `slice-01-within-status-reorder` | US-01 | Disproves "the chosen rank model (ODD-1) + a position-carrying persist (ODD-2/3) + an ordered read + a backfill migration is a clean, race-safe increment" if precision-exhaustion, renumber cost, concurrency, or the migration backfill needs machinery we lack. | Order cards within a column; it persists for everyone |
| 02 | `slice-02-cross-status-positional` | US-02 | Disproves "a cross-status drop can set state AND rank atomically over the shipped state persist + slice-01 machinery" if combining the state write and the position write into one gesture needs a transaction/endpoint shape we lack. | Drop a card into another column at an exact slot, in one motion |

Slice 01 carries **all the novelty** — the rank column + `0012` migration + the position-aware persist + the
ordered read + the `board-dnd.js` insertion-index logic. Slice 02 is a **smaller delta**: fold the already-proven
position into the already-shipped cross-column move so one gesture writes state + rank.

### Taste tests
- Slice 01: one migration + one persist + one read change + extend one JS file — a single end-to-end vertical on
  the real board (production data). Not "4+ new components". Thin, but carries the real uncertainty (rank model) —
  DESIGN de-risks it (ODD-1). ✓
- Slice 02: reuses slice-01's persist/read/JS + the shipped `change_issue_state`; the only new thing is
  atomic state+rank in one write. Thin. ✓
- The rank model is a shared abstraction both slices need → slice 01 ships it FIRST as its own end-to-end slice
  (not a bare abstraction). ✓
- Each slice disproves a real pre-commitment (rank model; atomic state+rank). Not decoration. ✓

## Prioritization
01 then 02. Slice 01 has the **highest uncertainty** (persisted rank model, precision/concurrency, migration
backfill) — sequence it first so a wrong rank-model bet is caught cheaply, before slice 02 builds on it. Slice 02
then reuses proven machinery. Dogfood each on the live sandbox board (the GEN-* cards).
