# Slice 02 delivery notes — the lane under a dragged card activates

Steps 02-01 and 02-02, both GREEN, 2026-09-14. No-commit mode. Production files
changed: `crates/foundry-app/static/js/board-dnd.js`, the stylesheet (renamed
`foundry.52ad52fa.css` → `foundry.ed2e1ba7.css`, sha256
`ed2e1ba71b2aa53e7a906827de5901c54de88a8b5841db896a4331c0fd9a1198`), and the D13
rename sites `templates/base.html`, the three cache-test literals in `src/lib.rs`
(literals only) and `static/VENDOR.md`.

## Mechanism (DDD-3, ADR-BOARD-CARD-002)

- `activate(lane)` sets/strips `data-card-drop-target` by live query, writing the
  DOM only on a change; the session branch of `dragover` is the only activator.
- A fifth delegated listener, `dragleave`, schedules a next-frame clear when
  `relatedTarget` is null ("left the window"); any `dragover` cancels it.
- `CardDragSession.prototype.end` calls `activate(null)`: one idempotent teardown
  at drop (before the POST answers), `dragend`, and a stale-session `dragstart`.
- Accepted deviation (02-01): while a session exists, `dragover` clears
  activation before returning off `#board-columns`, so the page header (outside
  the board) goes dark. Safe on other pages: a session can only begin inside the
  board. The browser default still applies off the board.

## OQ-1 — activated lane surface: `--cz-bg`

Rule: `.column[data-card-drop-target] { background: var(--cz-bg); outline: 2px
solid var(--cz-muted); outline-offset: -2px; }` — existing tokens only (S1), no
new token (D6), no layout (inset outline, the `.kb-selected` idiom).

`--cz-surface` was rejected: it equals the card surface in the light palette
(1.00:1), erasing card elevation. `--cz-bg` keeps cards raised in both palettes.

| Measurement | Light | Dark |
|---|---|---|
| Outline `--cz-muted` vs page `--cz-bg` (the oracle, ≥3:1) | 5.89:1 | 6.38:1 |
| Outline vs resting lane `--cz-bg-2` | 5.52:1 | 6.08:1 |
| Card `--cz-surface` vs activated lane `--cz-bg` | 1.04:1 | 1.09:1 |
| Card vs resting lane (reference) | 1.10:1 | 1.04:1 |
| Surface shift `--cz-bg` vs `--cz-bg-2` | 1.07:1 | 1.05:1 |

The rectangle oracle in "The activated lane is legible in both palettes and moves
nothing" confirms no card or column moves or resizes.

## Gates

| Gate | Result |
|---|---|
| us-cdf-02 | 8/8 scenarios, 13/13 examples |
| us-cdf-01 | 16/16 examples |
| Guards (KPI 8) | blr 26/26, kb 38/38, icd 36/36 — shipped files unmodified |
| Default lane | 632/632 |
| `cargo xtask smoke` / `check-arch` | green (hash = filename, VENDOR sha256, `/static` refs resolve) |
| Old-hash references | only the shipped comment `feature_issue_card_delete.rs:1571` (left by design, KPI 8) |
| Named fault M9 (clear on every `dragleave`) | **killed 1/1** — "A lane stays lit while the card passes over the cards inside it" RED on its oracle (`Done is not shown as activated … [data-card-drop-target] on lanes: []`); the other 12 examples stayed green as designed (they read after a `dragover`). Restored by `cp` + `cmp` (snapshot `board-dnd.js.green02`, sha256 `921cd1cd…`); us-cdf-02 13/13 after. cargo-mutants N/A still holds: `lib.rs` changed only its three rename literals |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`) |

## Dogfood (D12)

Orchestrator, Chrome, after `./restart.sh` (clean state proven first: `board-dnd.js`
`cmp`-identical to `board-dnd.js.green02`, no lane running), 2026-09-14. Board
`/team/general/project/sandbox`; no card created or deleted; restored to its start
state (GEN-2 in Backlog) and confirmed by reload.

The in-flight highlight cannot be screenshotted mid real drag (the automation's
mouse drag is atomic), so the visual states were driven with synthetic
`DragEvent`s into the real listeners — the same idiom as the tests — and the
drops themselves with the real mouse.

| Check | Result |
|---|---|
| New stylesheet served | ✓ the activation rule (only in `ed2e1ba7`) is live |
| Card over Done, light palette | ✓ only Done lit: `outline: solid 2px rgb(92,100,95) offset -2px` (`--cz-muted`, inset) on `rgb(251,251,249)` (`--cz-bg`); clearly visible, nothing moved |
| Moved on over In-Progress | ✓ only In-Progress lit |
| Over the page header (outside the board) | ✓ zero lit |
| Escape (`dragend` without `drop`) | ✓ zero lit |
| Dark palette (temporary `data-theme="dark"`, removed afterwards; the user's theme setting untouched) | ✓ only Done lit, outline `rgb(141,149,143)` over `rgb(10,12,11)`; clearly visible |
| Real mouse drag GEN-2 → Done, then → Backlog | ✓ both landed (`state=done`, `state=backlog`), zero lit after each drop, persisted after reload. One earlier attempt missed the card (estimated coordinates; no event recorded) and was retried at the measured card centre — not a product fault |
| Expected slice-04 gap | Placeholder still beside a dropped card until reload |

Still owed to the user: Firefox (real-mouse feel of the highlight, and whether it
flickers crossing a lane's own cards — this board has one card, so the
"over its own cards" feel was only proven synthetically, by "A lane stays lit
while the card passes over the cards inside it" and the M9 kill).
