# Evolution: fix-lane-menu-clipped-mobile (the lane ⋯ menu could not be touched on a phone)

**Finalized:** 2026-09-03.
**Commits:** `84d025e` (the fix, bundled with board-lane-overflow-menu and board-lane-reorder in
one commit, because the three shared a stylesheet hash chain and could not be split faithfully)
and `f73a3c9` (follow-up: the visible trigger failed WCAG 1.4.11 contrast). Both first released
in v0.4.0. RCA: `docs/feature/fix-lane-menu-clipped-mobile/rca.md`.
**Origin:** user report on 2026-09-03 with a screenshot from a real device: "the menu should show
up over the top of the list and the … should be easier to see and click."

## Defect: real, user-facing (phones)

- **A, clipped menu.** `.board { overflow-x: auto }` under `max-width: 480px` (from
  `pwa-mobile-rendering`) makes `.board` a clipping container for the absolutely-positioned menu.
  At 390px three of four items (Insert list before, Insert list after, Delete list) rendered but
  failed a hit test.
- **B, invisible trigger.** At rest the trigger was 27×32, transparent border and background;
  it gained a surface only on hover or focus, which a phone lacks. At 44px it overlapped the first
  card by 18px.
- **Why no test caught it:** every `@needs-browser` scenario ran at desktop width, where `.board`
  has no overflow.

## Fix

- The menu is `position: fixed`, positioned from the trigger's rect (`positionLaneMenu()` in
  `keyboard.js`), and closes on scroll and resize. No transform, filter or `contain: paint`
  ancestor exists to re-trap it.
- The trigger is a visible chip (`--cz-surface` + border at rest), 44×44 on mobile, inside a header
  band that reserves its space. One hairline separates "Delete list".
- `f73a3c9`: the chip's `--cz-line` border measured 1.15:1 light / 1.20:1 dark against WCAG
  1.4.11's 3:1. It now uses `--cz-muted` (5.52:1 / 6.08:1), and hover goes to `--cz-text`.

## Regression tests

Two `@needs-browser @mobile` scenarios in `board-lane-overflow-menu.feature`, both RED before the
fix, using real chromedriver mobile emulation:
- "Every menu item is reachable on a phone-sized screen": a hit test, not `is_displayed()`.
- "The menu trigger is visible at rest and big enough to touch": a visible edge at rest, ≥44×44,
  zero overlap with the first card.

The fix's own runs found three more issues (step 01-02): a mobile override ~200 lines above the
rule it overrode, `menu_is_open` probes broken by `offsetParent` being null for fixed elements, and
an unexplained headless-only empty `style=""` residue. The byte-identity oracle normalises that
residue on `[data-lane-menu]` only, with 3 unit tests.

## Gates

- Execution log: 01-01 and 01-02 record PREPARE, RED_ACCEPTANCE and GREEN; GREEN reports 25/25
  board-lane-overflow-menu scenarios.
- `84d025e`: fmt, clippy, check-arch, release build, workspace tests and `cargo deny` pass;
  acceptance `all` lane 726/734. Six failures were a `pg_dump` 14 vs 16 environment mismatch; two
  were the `.lane-menu-trigger` contrast defect this fix introduced, committed red on purpose.
- `f73a3c9`: canzan-theme lane 28/28; check-arch passes.
- **Not run:** mutation testing and the DISTILL consolidated reviewer gate (per `84d025e`).

## Audit gap

`deliver/execution-log.json` has **no COMMIT entries** for either step, and `deliver/` has no
`roadmap.json`. The log was not edited; this record is the only link from the steps to
`84d025e` and `f73a3c9`.

## Follow-ups

- The headless-only `style=""` residue is still unexplained (it did not reproduce headful).
- Mutation testing for this fix's changes was never run.
