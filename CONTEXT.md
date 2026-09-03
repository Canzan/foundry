# CONTEXT

## Current Task

**`board-lane-reorder`** — full pipeline DISCUSS→DESIGN→DISTILL→DELIVER run in one session, on top of
the still-uncommitted `board-lane-overflow-menu` + `fix-lane-menu-clipped-mobile` work.
**NOTHING IS COMMITTED.** A lane's order is now changeable: drag its column header (Pointer Events,
works on touch) or pick **Move list left / Move list right** from the same `⋯` menu — which grows
from four items to six. A move writes `lanes.position` only: zero issue rows, zero change events,
zero slug/label mutations. **No migration — still 0015.**

## Key Decisions

- **Insert's shuffle does NOT generalise to a move.** Insert is safe only because its bulk `+1`
  *vacates* the target slot; a move has no vacancy, so the shift collides with the mover still in its
  old slot. A move is therefore ONE `UPDATE … SET position = CASE …` statement. All three candidate
  shapes were measured against a real postgres:16-alpine, and **all three fail against a
  non-deferrable constraint** — `DEFERRABLE` is a *precondition* for lane reordering, not a
  convenience. See `adr-board-lane-006`.
- **The unlocked move race is SILENT** — no error, contiguity intact, uniqueness intact, and a lane
  nobody mentioned shoved past another (5/5 measured). So the concurrency oracle asserts the resulting
  **order**, never "no error raised" — the natural assertion passes on the corrupt case.
- **Two drag mechanisms on one board, deliberately** (`adr-board-lane-007`): lanes on Pointer Events
  (HTML5 drag emits nothing on touch), cards still on HTML5 DnD. Boundary is origin-based; the shipped
  card-drag scenarios passing *unmodified* are its standing proof.
- **`check-arch` now pins the `DEFERRABLE` keyword** (5 gold tests, one of which caught that the rule
  originally accepted a *commented-out* keyword — SQL uses `--`, not `//`).

## Next Steps

- **Commit** when wanted (nothing staged). Pre-commit gate is the full `cargo xtask ci`.
- **Full gate run done**: fmt/clippy/check-arch/build/deny/workspace-tests (44 binaries) all PASS;
  acceptance `all` lane
  **726/734**. The 8 failures are NOT from this feature — 6 are `pg_dump 14.24` vs a 16 server
  (install `postgresql@16`), 2 are a **real WCAG 1.4.11 contrast failure on `.lane-menu-trigger`
  (1.20:1 / 1.15:1 vs 3:1)** from the uncommitted `fix-lane-menu-clipped-mobile` work — fix before
  committing that.
- Still not run: **mutation testing**, and the DISTILL consolidated 4-wave reviewer gate (Agent
  dispatch is disabled by user instruction).
- Successors: converge card drag onto Pointer Events (would give cards a touch drag too); undo a lane
  delete; "Sort by".
