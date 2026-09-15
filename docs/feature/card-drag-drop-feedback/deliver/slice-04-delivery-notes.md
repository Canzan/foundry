# Slice 04 delivery notes — empty lanes read truthfully

Steps 04-01 and 04-02, both GREEN, 2026-09-14. No-commit mode. **Slice 04's
hypothesis held:** 04-02 un-pended the refusal, hover/cancel/foreign, and both
two-tab remote-delete scenarios, and all four were GREEN on arrival with NO
production edit — the declarative `:has()` rule alone keeps every lane truthful,
so ADR-BOARD-CARD-003's `<template>` fallback never arose.
Production files changed: `crates/foundry-app/templates/partials/board_columns.html`,
the stylesheet (renamed `foundry.54eb7a9b.css` → `foundry.f7c36a08.css`, sha256
`f7c36a0871d8ab594b6b8767d957b4fbb3eb977b81226feda2b1ed80fb87a3eb`), and the D13
rename sites `templates/base.html`, the three cache-test literals in `src/lib.rs`
(literals only) and `static/VENDOR.md`. `board-dnd.js` and `board-live.js` are
unchanged in this slice.

## Mechanism (DDD-5, ADR-BOARD-CARD-003)

- The partial drops its `{% if column.cards.is_empty() %} … {% else %}` branch:
  EVERY `section.column` emits the unchanged `<p class="empty">No issues yet — press
  <kbd>c</kbd> to file the first one.</p>` before its cards. The OOB refresh includes
  the same partial, so both render paths stay byte-identical.
- The stylesheet's colour-free rule pair `.column > .empty { display: none }` /
  `.column:not(:has(> .issue-card)) > .empty { display: block }` displays it only
  while the lane holds no card. Zero script writers: the optimistic drop, the
  identity revert and `board-live.js`'s remote delete are truthful by construction.
- Below the `:has()` floor (Chrome/Edge 105, Safari 15.4, Firefox 121) the
  placeholder never shows — silence, not a lie. `<template>` stays the documented
  fallback only.
- The crafter added a four-line template comment warning against restoring the
  old if/else (Askama strips it; rendered HTML unaffected) — kept.

## Gates

| Gate | 04-01 | 04-02 |
|---|---|---|
| us-cdf-04 | 4/4 un-pended scenarios (#27, #28, #29, #34), RED first on the placeholder oracle | **8/8 scenarios, 10/10 examples** (#30–#33 green on un-pending) |
| cdf | 48/48 executed (#30–#33 still `@pending`) | **54/54, zero `@pending` left** |
| Guards (KPI 8) blr / kb / icd / blm / blo | 26/26 / 38/38 / 36/36 / 24/24 / 25/25 | 26/26 / 38/38 / 36/36 / 24/24 / 25/25 |
| Default lane | 632/632 | first run 631/632 — the documented sqlx `unknown message type '\0'` concurrent-testcontainers flake, here in `notification-delivery-providers.feature:62` while connecting to the base database (feature untouched; passes 30/30 alone); full re-run **632/632**. The crafter correctly logged GREEN `FAIL` under its too-narrow brief; the orchestrator accepted the flake on this evidence and the crafter appended GREEN `PASS` |
| `cargo test --workspace` | green; `projects.rs:1157` (placeholder still rendered on an empty board) ok. First run 631/632 on a one-off 90s browser-container readiness timeout after a Gatekeeper stall; clean re-run green | |
| `cargo xtask smoke` / check-arch | green (hash = filename, VENDOR sha256) | _pending_ |
| Named faults M7 / M8 | — | **M7 killed** (`:has()` pair removed → "One drag updates both lanes at once" RED: `Staging still displays the "No issues yet" placeholder beside a card`; also "A delete in another tab that leaves cards behind adds no placeholder" RED). Its named "A lane emptied by a delete in another tab shows the placeholder" **survived** — it never asserted the placeholder was hidden before the delete — and the DISTILL owner **CLOSED** it in-slice: a visible Given `In-Progress in the second tab does not show the placeholder while it holds OPS-7` (feature:375; step `given_second_tab_no_placeholder_while_holding`); re-seeded M7 now reddens it (`In-Progress still displays the "No issues yet" placeholder beside a card … ("in_progress", ["OPS-7"], true, …)`); restored by `cp` + `cmp`, us-cdf-04 10/10, `cdf` 54/54 (400 steps). **M8 killed**, all three named scenarios RED on `… must be back in its exact origin slot` after proving the move was sent and refused. Both files restored by `cp` + `cmp`; `cdf` 54/54 after. **Feature gate: 9/9 killed** |
| Browser | Chrome 151.0.7922.108 | |

## Dogfood (D12)

Orchestrator, Chrome, after `./restart.sh` (clean state proven first: the stylesheet
and `board-dnd.js` `cmp`-identical to their green04 snapshots, no lane running),
2026-09-14. Board `/team/general/project/sandbox` (Backlog GEN-4, GEN-3, GEN-2; the
two temp issues from slice 03 reused). Because the automation tool's real-mouse drag
proved unreliable in slice 03, each move used synthetic `DragEvent`s into the real
listeners (the tests' idiom); every drop sent the real `POST …/state`. The
placeholder oracle is the computed `display` of each lane's `.empty`.

| Moment | Backlog | In-Progress | Done |
|---|---|---|---|
| Before | 3 cards · placeholder `none` | empty · `block` | empty · `block` |
| GEN-4, GEN-3, GEN-2 dropped into Done | empty · **`block`** | empty · `block` | 3 cards · **`none`** |
| Reload | identical to the row above | | |
| All three dropped back into Backlog, then reload | 3 cards (GEN-4, GEN-3, GEN-2) · `none` | empty · `block` | empty · `block` |

All six drops were claimed; the board was left exactly as found. **Still owed to
the user:** a remote delete emptying a lane (it needs a real issue deleted — the
orchestrator does not hard-delete data; the automated scenario covers it and is
being strengthened for the M7 gap), a refused drop by hand, and Firefox + Safari
(`:has()` in two more engines).
