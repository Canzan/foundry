# Slice 01 delivery notes — drops keep working after an in-place refresh

Steps 01-01 and 01-02, both GREEN, 2026-09-14. No-commit mode. Only production
file changed: `crates/foundry-app/static/js/board-dnd.js`.

## Gates

| Gate | Result |
|---|---|
| us-cdf-01 lane | 9/9 scenarios, 16/16 examples |
| Guards (KPI 8) | blr 26/26, kb 38/38, icd 36/36, layered 3/3 — shipped files unmodified |
| Default lane | 632/632 |
| `cargo xtask smoke` | green |
| Named faults (DDD-9) | M1, M2, M3 killed — 3/3 (`mutation/mutation-report.md`) |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`) |

Open follow-up from the fault gate: the swallow outline did not assert
`dropEffect == "none"`, so M2 reddened only the other-tab scenario. The user
chose to add the assertion (in progress, DISTILL-owned).

## Dogfood (D12) — orchestrator, Chrome, `./restart.sh` dev server, 2026-09-14

Board `/team/general/project/sandbox` (GEN-2 in Backlog; In-Progress and Done empty).
No card was created or deleted; the board was restored to its starting state.

| Check | Result |
|---|---|
| Lane ⋯ **Move list right** → `#board-columns` replaced in place (old node detached, page not reloaded) | ✓ |
| Real mouse drag GEN-2 → empty Done, no reload | ✓ `POST …/issues/2/state state=done`; persisted after reload |
| Lane ⋯ **Move list left** → second in-place replace | ✓ |
| Real mouse drag GEN-2 → empty Backlog, no reload | ✓ `state=backlog`; persisted after reload |
| Foreign file (synthetic `File` in `DataTransfer`) on a lane and on the gap, fresh load and after a replace | ✓ `dragover`/`drop` `defaultPrevented`, 0 requests, URL unchanged. **Correction:** the `dropEffect "none"` read here proves nothing: Chrome reports `"none"` for any script-built `new DataTransfer()` regardless of what the page sets (found while closing the M2 gap). The no-drop cursor is proven instead by the swallow outline's new step "the board shows that it will not take the drop", whose drag kit gives the foreign transfer a writable `dropEffect` starting at `"copy"`; M2 now reddens all 5 rows. |
| Expected slice-04 gap (not a slice-01 defect) | Placeholder stays beside a dropped card; emptied lane shows none until reload |

**Still owed to the user (cannot be automated here):**

- A **real Finder file** (`keys.png`) dropped on a lane and on the gap, fresh
  load and after a refresh — in **Chrome, Firefox and Safari** (DESIGN OQ-2).
  If any browser opens the file, apply ADR-BOARD-CARD-001's fallback (accept with
  `move`, cancel in `drop`).
- Real-mouse feel in **Firefox and Safari**.

## KPI 2 log — refused drops over 5 working days (dogfood-only instrument)

| date | refused drops | pre-emptive reloads | notes |
|---|---|---|---|
| | | | |
