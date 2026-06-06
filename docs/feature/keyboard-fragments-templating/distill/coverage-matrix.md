# DISTILL Coverage Matrix — keyboard-fragments-templating

> LEAN wave. Move-only, inherit-only. Reconciliation gate PASSED (0 contradictions).
> `[lang-mode] rust` · `[policy-mode] inherit` · `[port-mode] n/a (Rust; no state_delta port — see below)`

## The no-delta rationale (why this wave is LEAN)

US-K01 (search-results fragment) and US-K02 (keyboard-help overlay) move two
inline-`format!()` **BARE htmx fragments** in `keyboard.rs` to Askama partials.
They are **byte-identical** after the move: no `base.html`, no new `/static`
link, **no new observable user-facing delta**.

Consequence for acceptance design: the move is proven by the **existing
us-12-keyboard-nav suite staying green** (the regression net), NOT by new
"renders styled" scenarios. Re-asserting the existing markup would be redundant
Fixture-Theater-adjacent noise. So this wave adds the **minimum genuine deltas**
only:

1. Two **regression-net-tightening** scenarios for the render-contract markers
   us-12 leaves thin (pass today, stay green after the move).
2. One **completion guard** (the genuine new RED): assert no inline-`format!()`
   fragment HTML remains in `keyboard.rs`.

## Surface → coverage decision

| Story | Surface (`keyboard.rs`) | us-12 already pins | us-12 GAP | DISTILL decision |
|---|---|---|---|---|
| US-K01 | `render_search_fragment` (~:230) | `li.search-result[data-issue-key]`, `.title` text, exact-key match, substring match | `ul.search-results` wrapper, `.key` span, empty `ul.search-results[data-empty="true"]` | **regression-net + 1 gap scenario** (wrapper + key + empty state) |
| US-K02 | `show_keyboard_help` (~:252) | each `dt[data-shortcut]`+`dd` pair (7 shortcuts), "valid HTML fragment" | `section.keyboard-help[role="dialog"][aria-label]` container, `header>h2` heading | **regression-net + 1 gap scenario** (dialog container + heading) |
| US-K01 + US-K02 | both literals in source | — (us-12 is a runtime contract; no source-tree check) | inline-`format!()` literals are unguarded by a completion check | **completion guard** (the new RED — 3 source sites) |

Both surfaces are **regression-net-only for behaviour** (us-12 proves the markup
is preserved end-to-end through real HTTP). The gap scenarios exist purely to
tighten two render-contract markers us-12 never asserted, so the move cannot
silently drop the wrapper / key / empty-state / dialog-container / heading.

## Story → scenario traceability

| Story | Scenario | Tags | Layer | Status today |
|---|---|---|---|---|
| US-K01 | (regression net) all 4 us-12 search scenarios | `@us-12 @real-io` | 4 (real HTTP, real PG) | GREEN (unchanged) |
| US-K01 | A search match renders inside the search-results list with a key and a title | `@us-k01 @real-io` | 4 | GREEN (net tighten) |
| US-K01 | A search with no matches renders the empty search-results list | `@us-k01 @real-io` | 4 | GREEN (net tighten) |
| US-K02 | (regression net) us-12 keyboard-help shortcut-enumeration scenario | `@us-12 @real-io` | 4 | GREEN (unchanged) |
| US-K02 | The keyboard-help overlay is a labelled dialog with a heading | `@us-k02 @real-io` | 4 | GREEN (net tighten) |
| US-K01 + US-K02 | No inline format!() HTML remains in the keyboard surfaces | `@us-k01 @us-k02 @completion-check @source-tree` | source-tree | **RED now (3 sites) → GREEN after DELIVER** |

Every story (US-K01, US-K02) has at least one scenario. No story is uncovered.

## Adapter / driving-port coverage (Mandate 6, inherit-only)

| Port / adapter | Treatment | Covered by |
|---|---|---|
| HTTP API `GET /team/{t}/project/{p}/search?q=` (driving) | real `reqwest` → `InProcHarness` (ATDD policy: in-process `spawn_app`) | us-12 + 2 US-K01 scenarios `@real-io` |
| HTTP API `GET /keyboard-help` (driving) | real `reqwest` → `InProcHarness` | us-12 + 1 US-K02 scenario `@real-io` |
| `PgPool` (driven internal) | real testcontainers PG16 + per-scenario schema | exercised by the search scenarios (seeds + filters real issues) |
| Askama template render (driven internal) | real render through the in-process router | proven GREEN end-to-end by us-12 + gap scenarios after the move |
| source-tree scan of `keyboard.rs` (contract) | real `std::fs::read_to_string` (mirrors `vendored_htmx_files` / `inline_full_page_sites`) | completion-guard scenario |

No NEW driven adapter is introduced → no new `@adapter-integration` scenario
required (Mandate 6 satisfied by inheritance).

## Error / edge coverage

The genuine edge for these fragments is the **empty search result** (`data-empty="true"`),
which us-12 never exercised — now covered by the "no matches" gap scenario. The
help overlay has no error path (it is a static public overlay). Error-path ratio
is dominated by the inherited us-12 net + remaining-surfaces error fragments;
this LEAN delta adds the one missing edge (empty state) the move must preserve.

## Mandate / pillar notes (LEAN)

- **Mandate 8 (state-delta / Universe)**: N/A. All scenarios run at **layer 4**
  (real HTTP, real PG, or source-tree scan). Mandate 8 binds layers 1–3;
  layer 4+ uses traditional assertions (here: `scraper` selector asserts +
  source-site count). No Rust `state_delta` port is bootstrapped — no
  state-mutating layer-1–3 step exists in this delta.
- **Mandate 9 (PBT mode)**: example-only (all scenarios layer 3+). No PBT
  machinery — correct per the layer table.
- **Mandate 10 (Tier B)**: SKIP. Journey is 2 short surfaces, config-shaped
  move-only refactor, no domain-rich input space, no ≥3-chained journey. Tier A
  (cucumber `.feature` + steps via the production in-process router) only.
- **Mandate 11 (sad paths example-based)**: the empty-state scenario is a named
  example, not PBT-generated. Correct.
- **Pillar 1 (domain language)**: scenario titles/steps speak "search-results
  list", "labelled dialog", "heading", "inline fragment HTML" — no HTTP/JSON/DB
  jargon in Gherkin.
- **Pillar 2 (chained narrative)**: the search gap scenario reuses the us-12
  `Given the "..." project already has an issue titled "..."` + `When ... searches`
  step-methods (step composition, not copy-pasted fixtures).
- **Pillar 3 (production composition)**: SUT is the real in-process axum router
  (`InProcHarness`/`spawn_app`), per the ATDD Infrastructure Policy. Only the
  clock/email/etc. are faked (none relevant here).
