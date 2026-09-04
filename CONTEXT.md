# CONTEXT

## Current Task

**`board-lane-reorder` SHIPPED** on branch `board-lane-shaping` — 7 commits, clean tree, **NOT
PUSHED**. A board's lane order is changeable: drag a column header (Pointer Events, works on
touch) or pick **Move list left / right** from the `⋯` menu, now six items with disabled ends.
A move writes `lanes.position` only — zero issue rows, zero change events, zero identity
mutations. **No migration; still 0015.** The commit also carries the previously-uncommitted
`board-lane-overflow-menu` and `fix-lane-menu-clipped-mobile` work (entangled via one stylesheet
hash chain, so a per-feature split was not reconstructable).

## Key Decisions

- **Insert's shuffle does NOT generalise to a move** — insert *vacates* the target slot, a move
  has no vacancy. One `UPDATE … SET position = CASE …` statement. All three candidate shapes
  fail against a non-deferrable constraint, so `DEFERRABLE` is a **precondition**, now pinned by
  a `check-arch` rule with 5 gold tests (`adr-board-lane-006`).
- **The unlocked move race is SILENT** — no error, invariants intact, board arranged as nobody
  asked. So the concurrency oracle asserts resulting **order**, never "no error raised".
- **Host tool dependencies removed**: `pg_dump`/`pg_restore` and chromedriver/Chrome now run from
  containers pinned to the server's own image tag, so version skew is impossible rather than
  detected. `xtask ci` preflights 2 and 3 retired with them.

## Next Steps

- **Push** when wanted. NB `8b79448` on this branch came from another session, not this work.
- **Re-run the full `all` lane**: last measured 734/734 BEFORE the two review-driven scenarios
  landed. `blr` is 26/26; the full number is expected-but-unverified at 736.
- **Reap 5 orphaned testcontainers** (21–29h old) — the likely cause of three `foundry-store`
  tests flaking under load, each passing in isolation. Left alone; another session may own them.
- Still running: foundry on your tailnet — `kill 72826 && docker rm -f foundry-dev-pg`.
