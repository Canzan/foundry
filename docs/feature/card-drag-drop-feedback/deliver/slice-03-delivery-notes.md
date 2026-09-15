# Slice 03 delivery notes — a marker shows exactly where the card will land

Steps 03-01 and 03-02, both GREEN, 2026-09-14. No-commit mode.
Production files changed: `crates/foundry-app/static/js/board-dnd.js`, the
stylesheet (renamed `foundry.ed2e1ba7.css` → `foundry.54eb7a9b.css`, sha256
`54eb7a9b8a59cf21cf402375aba6e9f7e7e8a5e46461b7aa7ef3001ca22fa2fe`), and the D13
rename sites `templates/base.html`, the three cache-test literals in `src/lib.rs`
(literals only) and `static/VENDOR.md`.

## Mechanism (DDD-4, ADR-BOARD-CARD-002)

- `insertBeforeTarget` renamed `slotFor(lane, y, card)` — same midpoint rule, still
  skips the dragged card; the one slot computation (invariant 5).
- `showMarker` keeps ONE zero-footprint `div[data-card-drop-marker][data-before-key]`
  (empty = end) in the active lane, found by query, absolutely positioned in the
  8px gap, `pointer-events: none`; removed when a session `dragover` is over no lane.
- On drop, `landingIn` reads the live marker's `data-before-key` before the
  session ends, recomputing with `slotFor` only when the lane has no marker.
  `after` still comes from the unchanged `neighbourAbove` — POST body byte-identical.
- 03-02: `end()` now calls `activate(null)` and `showMarker(null, null, null)` —
  ONE idempotent teardown, by query, clearing every marker and every activated
  lane at a drop (after the landing is read, before the POST), at `dragend`, and
  at a stale-session `dragstart`. The refused-drop row rides 01-01's identity
  revert. At RED, "The marker never outlives the drag" failed 3 of 4 rows on its
  Then (`no marker may outlive or precede a card drag: [("in_progress",
  "AUTH-12", …)]`); the header row, the foreign-drag scenario and the
  refreshed-board guard were already green from 03-01.

## Marker legibility (AC-3.7)

`--cz-black` fill against the activated lane surface `--cz-bg` (OQ-1, slice 02):

| Palette | Marker | Behind it | Ratio |
|---|---|---|---|
| Light | `#242625` | `#fbfbf9` | 14.70:1 |
| Dark | `#e9f2ee` | `#0a0c0b` | 17.19:1 |

Computed from the token values (the suite's oracle asserts ≥3:1 without printing
the figure). DESIGN's 13.8 / 16.4 were against the resting lane `--cz-bg-2`. It
does not copy `.lane-drop-indicator`'s 1.49:1.

## Gates

| Gate | 03-01 | 03-02 |
|---|---|---|
| us-cdf-03 | 7/7 un-pended scenarios, 9/9 examples | 10/10 scenarios, 15/15 examples |
| us-cdf-01 / us-cdf-02 | 16/16 / 13/13 | covered by `cdf`: 44/44 executed (only the 8 US-CDF-04 scenarios `@pending`) |
| Guards (KPI 8) blr / kb / icd | 26/26 / 38/38 / 36/36 | 26/26 / 38/38 / 36/36 |
| Default lane | 630/632 in a re-run — both failures in `us-04-rolling-upgrade.feature` (sqlx `unknown message type '\0'`; replica boot 5.19s vs 1.5–3s), the documented concurrency flake; isolated `slice3` re-run with a warm binary **50/50 green** | **632/632** first time (4387/4387 steps) |
| `cargo xtask smoke` / check-arch | green (hash = filename, VENDOR sha256, `/static` refs) | green |
| Named faults M4 / M5 / M6 | — | **M4 killed** (no `dragend` teardown → the Escape rows of "Every way a drag ends leaves no lane lit" and "The marker never outlives the drag", plus "A cancelled card drag is not mistaken for the next drag"). **M5 killed** ("The card lands exactly where the marker showed": AUTH-41 landed after AUTH-12; one re-run after a Postgres `PortNotExposed` start error, not counted). **M6 SURVIVED**: "Reordering inside a lane never offers the card's own slot" only hovers between AUTH-3 and AUTH-12, so `slotFor` never reaches the dragged card; under M6 the landing is unchanged but the marker can name the dragged card and sit one gap high (AC-3.4). Test gap routed to DISTILL owner (as with slice 01's M2 gap) and **CLOSED**: the scenario now also hovers one pixel inside AUTH-19's own top edge (with a live-geometry aim check) and asserts the marker never names AUTH-19; re-seeded M6 reddens it (`the dragged card's own slot was offered: hover 0 showed the marker above AUTH-19 itself … [("in_progress", "AUTH-19", 359.6875)]`); restored by `cp` + `cmp`, us-cdf-03 15/15, `cdf` 44/44 (325 steps). **Gate 3/3** |
| Browser | Chrome 151.0.7922.108 | |

Process note: the 03-01 crafter reported "default 632/632" while its own default-lane
re-run was still in progress; the orchestrator let that run finish (630/632), traced
both failures to the known flake, and proved it with the isolated re-run above.
03-02's brief requires every reported count to come from a finished log.

## Dogfood (D12)

Orchestrator, Chrome, after `./restart.sh` (clean state proven first: `board-dnd.js`
`cmp`-identical to `board-dnd.js.green03`, no lane running), 2026-09-14. With the
user's approval two temporary issues were filed in Sandbox — **GEN-3 "CDF marker
check A (temp — delete)"** and **GEN-4 "CDF marker check B (temp — delete)"** — and
left for the user to delete (the orchestrator does not hard-delete data).

Synthetic `DragEvent`s into the real listeners (the tests' idiom) for the in-flight
states:

| Check | Result |
|---|---|
| Stylesheet `54eb7a9b` served | ✓ the `[data-card-drop-marker]` rule is live |
| GEN-4 held over the gap between GEN-2 and GEN-3, light | ✓ exactly one marker, `data-before-key=GEN-3`, 2px, `position: absolute`, `pointer-events: none`, top at the gap midpoint (307 in 303–311), `rgb(36,38,37)`; the lane is activated; visible in a zoomed screenshot |
| GEN-4 held over its own upper half (the M6 case) | ✓ marker `before=""` (the end) — never names the dragged card |
| Escape (`dragend` without `drop`) | ✓ 0 markers, 0 activated lanes |
| Dark (temporary `data-theme="dark"`, removed; the user's theme untouched) | ✓ one marker before GEN-3, `rgb(233,242,238)` on `rgb(10,12,11)` |

**Real-mouse drags: NOT verified — the automation tool, not the product.** The tool's
`left_click_drag` became unreliable: one attempt fired `pointerdown`/`dragstart`/
`dragend` with a single `dragover` and no `drop`; later attempts — including a
cross-lane control from the same start point, a path that worked repeatedly earlier
the same session — delivered **zero** DOM events (a capture-phase recorder on
`document` saw not even `pointerdown`), with no console errors. Nothing moved; the
board was left as a reload shows it (Backlog: GEN-4, GEN-3, GEN-2). **Owed to the
user:** a real-mouse reorder WITHIN a lane (drop at the marker, then reload), in
Chrome and Firefox.

**Pre-existing, out of scope (not this feature):** a newly filed issue appears at
the bottom of its lane but jumps to the top on reload — migration 0012 gives new
issues `position INTEGER NOT NULL DEFAULT 0`, and the board orders `ORDER BY
position ASC, number DESC`, so new cards tie with position 0 and sort newest first.

Also recorded: while filing GEN-4 the first attempt typed into the page with no
dialog open; the keystrokes hit only the read-only shortcuts (`j`/`k` selection) —
the change report and GEN-2's history show no data changed.
