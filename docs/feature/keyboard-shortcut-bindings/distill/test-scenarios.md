# DISTILL — Test Scenarios: keyboard-shortcut-bindings

> Quinn (nw-acceptance-designer), DISTILL wave. Executable SSOT = the cucumber-rs feature file
> `crates/foundry-acceptance/tests/features/keyboard-shortcut-bindings.feature` (38 scenarios)
> + the RED-ready `@pending` panic scaffold
> `crates/foundry-acceptance/src/steps/keyboard_shortcut_bindings.rs`. This document is the
> scenario -> US/AC/ADR map, the harness-boundary rationale, the resolution of the two
> DISTILL-open AC notes, and the precise `@needs-browser` lane wiring DELIVER must execute
> without rediscovery.

## Prior-wave reading checklist (Mandate: read every input)

DESIGN (the binding contract):
- ✓ `design/architecture.md` (C4 L1/L2/L3, resolved contracts, reuse-vs-new 26 rows, enforcement)
- ✓ `design/adr-001-vanilla-dispatch-layer.md` (vanilla IIFE, `[data-kb-ready]`, drop Alpine, R9 doc fix)
- ✓ `design/adr-002-guard-predicate.md` (the crux: 4-step chain, `isTextEntry`, IME, Shift not a suppressor)
- ✓ `design/adr-003-overlay-host-and-layer-stack.md` (`#kb-overlay-root`, DOM-derived Esc stack, keep no-JS links)
- ✓ `design/adr-004-key-based-selection.md` (`selectedKey`, ring derived, drag/swap coherence)
- ✓ `design/adr-005-search-surface-and-enter-resolution.md` (board-only, injected panel, Enter-via-board-card)
- ✓ `design/adr-006-selection-accessibility.md` (RATIFIED Option A: `aria-activedescendant`, conditional KPI-4)
- ✓ `design/adr-007-browser-e2e-harness.md` (fantoccini lane, probe-then-refuse — the spine of DISTILL)
- ✓ `design/adr-008-retire-kb-items.md` (13-site map, trap A rename, trap B vacuity)
- ✓ `design/wave-decisions.md` (DD1-DD9, ODD→ADR map, the two AC re-shaping notes left open for DISTILL)
- ✓ `design/upstream-changes.md` (§1 harness-already-serves, §2 no-Playwright reversal, §3 D2 amendment, §4 search hx-get)
DISCUSS:
- ✓ `discuss/user-stories.md` (US-01..07, embedded AC, Elevator Pitches)
- ✓ `discuss/acceptance-criteria.md` (AC tables; US-02 D15 inverted paired-assertion)
- ✓ `discuss/wave-decisions.md` (D1-D15; reconciliation source)
- ✓ `discuss/journey-keyboard-nav.feature` (DISCUSS journey sketch — reconciled, not copied; see §Reconciliation)
- ✓ `slices/slice-01..05-*.md` (slice boundaries — mapped to `@slice1`..`@slice5`)
- ✓ `AGENTS.md` (dead-code policy; new checks go in `cargo xtask ci`, never `ci.yml` alone)
Harness (mirrored for structure/voice):
- ✓ `tests/features/us-12-keyboard-nav.feature` (sibling; the no-Playwright decision + `@manual` drill this reverses/supersedes)
- ✓ `tests/features/recipient-notification-preferences.feature` + `notification-delivery-providers.feature` (recent DISTILL layout)
- ✓ `src/steps/us_12_keyboard_nav.rs` (the module extended; `#kb-items` assertions ADR-008 retires)
- ✓ `tests/acceptance.rs` :125-256 (lane selection to extend with `@needs-browser`)
Not found / not applicable:
- ⊘ `docs/feature/keyboard-shortcut-bindings/devops/` — no DEVOPS wave (consistent with prior trunk features; default env assumed)
- ⊘ `docs/product/` SSOT — house legacy multi-file convention (D14); no journeys.yaml / brief.md / kpi-contracts.yaml
- ⊘ `docs/feature/keyboard-shortcut-bindings/{discover,diverge,spike}/` — do not exist (upstream-changes §Predecessor lineage)

## Wave-Decision Reconciliation HARD GATE — PASSED (0 unresolved contradictions)

Read `discuss/wave-decisions.md` + `design/wave-decisions.md` (no DEVOPS wave). Cross-checked every
DISCUSS decision against DESIGN:

| DISCUSS | DESIGN | Verdict |
|---|---|---|
| D2 "board + issue list" | ADR-005 / upstream-changes §3 "BOARD ONLY" | **Reconciled amendment, user-ratified** — not a contradiction. The "issue list" was verified non-existent (`…/issues` POST-only, `lib.rs:487-490`); search is a panel ON the board. Scenarios follow board-only. |
| D4 "NOT roving tabindex; a11y needs an explicit answer" | ADR-006 Option A `aria-activedescendant`, D-4 stands | **Reconciled** — ODD-7 escalated + ratified 2026-07-15. KPI-4 conditional-on-Tab. |
| D8 "NO walking skeleton" | architecture "no product skeleton" | **Consistent** — DISTILL's skeleton is the lane-probe (instrument proof), not a product WS. |
| D9 "every AC reds on main" + D15 "US-02 inverts it" | architecture §Maintainability preserves D15 | **Consistent** — the one paired-assertion inversion is preserved, not "fixed". |
| D10 "zero server delta" | upstream-changes §6 clarifies (script tag + CSS re-hash + test infra are not handler deltas) | **Consistent** — no server behaviour scenario; the `@grep-litmus` proves removals. |

No decision in DISCUSS is contradicted by DESIGN. **Reconciliation passed — 0 contradictions.** The D2
amendment is a documented, ratified refinement recorded in `upstream-changes.md`, so it does not trigger
the CLARIFICATION_NEEDED block.

## Reconciliation with the DISCUSS journey sketch (`discuss/journey-keyboard-nav.feature`)

The DISCUSS sketch is a starting point, reconciled — NOT copied — against DESIGN:
- **US-06 `Enter`**: the sketch's "opens the selected issue" is kept, but a NEW scenario pins the ADR-005
  correction — `/` -> AUTH-2 -> `j` -> `Enter` opens the SAME modal a pointer click on the board card
  produces (search rows carry no `hx-get`), plus the named-edge no-op for an off-board-state issue.
- **US-02 `Cmd+C`**: the sketch's "the text is copied" is REPLACED (unassertable headless) by
  non-activation + `defaultPrevented===false` for BOTH Ctrl and Meta.
- **US-05 a11y**: the sketch's "the newly selected issue is announced" is made concrete as
  `aria-activedescendant` + `aria-selected`, with the "once the board is focused" qualifier and the
  help-copy obligation.
- **Added** (not in the sketch): the `@lane-probe` walking-skeleton-equivalent, the explicit `Shift+/`
  scenario, the explicit IME scenario, the layered-Esc second press, and the `@grep-litmus` retirement.

## The two DISTILL-open AC notes — RESOLVED (design/wave-decisions.md "Constraints for DISTILL")

1. **AC-02.3 `Cmd+C` / `Ctrl+C` (clipboard unassertable headless).** RESOLVED in scenario
   *"A copy chord does not create an issue and is left for the browser to handle"* (`@slice2 @modifier`):
   press the copy chord with **Ctrl** and again with **Cmd**; assert (a) the new-issue modal does NOT open
   for either, and (b) `keydown.defaultPrevented === false` for either — i.e. the layer is INERT and leaves
   the chord to the browser. It does **not** assert clipboard contents. A Gherkin comment records that Linux
   CI's copy chord is Ctrl, not Meta, which is why both are asserted.
2. **The IME clause (WebDriver `send_keys` cannot compose).** RESOLVED in scenario *"A key delivered mid IME
   composition does not fire a shortcut"* (`@slice2 @ime`): DELIVER drives it by `client.execute()`
   dispatching a `CompositionEvent` + `KeyboardEvent{isComposing:true, keyCode:229}` for `c`. A Gherkin
   comment states this SIMULATES rather than truly exercises an IME; the residual real-IME risk is carried
   by the `@manual` scenario.

## Scenario -> US / slice / AC / ADR map (38 scenarios)

| # | Scenario (abbrev) | Slice | Tags | US | AC | ADR |
|---|---|---|---|---|---|---|
| 1 | Browser lane drives a real key end to end | 01 | needs-browser, lane-probe, walking_skeleton, driving_port, real-io | US-01 | NFR-1 instrument | ADR-007/001/003 |
| 2 | `?` shows the list over the current page | 01 | needs-browser, help | US-01 | AC-01.1 | ADR-003 |
| 3 | Overlay lists exactly the seven; each bound | 01 | needs-browser, help, property, contract | US-01 | AC-01.2, AC-X.1 | ADR-001 |
| 4 | `?` available away from the board | 01 | needs-browser, help, global, edge | US-01 | AC-01.3 | ADR-003 |
| 5 | Dismissing help returns Mei where she was | 01 | needs-browser, escape | US-01/07 | AC-01.4 | ADR-003 |
| 6 | No-JS full-page help still works | 01 | needs-browser, no-js, property | US-01 | AC-01.5, AC-X.3 | ADR-003 |
| 7 | **No shortcut fires from a text field, still fires outside** | 02 | needs-browser, guard, critical, property, **paired-assertion** | US-02 | AC-02.1/02.2/02.7 | ADR-002 |
| 8 | Typing letters into a title inserts them | 02 | needs-browser, guard, edge | US-02 | AC-02.1 | ADR-002 |
| 9 | Copy chord does not create (Ctrl+Cmd, non-activation) | 02 | needs-browser, guard, modifier, error | US-02 | AC-02.3 (reshaped) | ADR-002/007 |
| 10 | Shift is not a suppressor (`?` fires) | 02 | needs-browser, guard, shift | US-02 | AC-02.4, BR-7 | ADR-002 |
| 11 | Key mid IME composition does not fire | 02 | needs-browser, guard, ime, edge | US-02 | AC-02.1 (IME) | ADR-002/007 |
| 12 | Leaving the field re-enables shortcuts | 02 | needs-browser, guard | US-02 | AC-02.6 | ADR-002 |
| 13 | `c` opens the new-issue modal, title focused | 03 | needs-browser, create | US-03 | AC-03.1 | ADR-005 |
| 14 | File an issue entirely from the keyboard | 03 | needs-browser, create | US-03 | AC-03.2 | ADR-005 |
| 15 | `c` does nothing where there is no project | 03 | needs-browser, create, scope, edge | US-03 | AC-03.3 | ADR-005 |
| 16 | `Esc` closes the new-issue modal | 03 | needs-browser, escape | US-03/07 | AC-07.1 | ADR-003 |
| 17 | **Layered Esc: help before the modal beneath** | 03 | needs-browser, escape, layered, critical | US-07 | AC-07.1 | ADR-003 |
| 18 | `Esc` with nothing open is a no-op | 03 | needs-browser, escape, edge | US-07 | AC-07.2 | ADR-003 |
| 19 | `/` focuses search box, empty (no slash) | 04 | needs-browser, search | US-04 | AC-04.1 | ADR-005 |
| 20 | Find by title substring | 04 | needs-browser, search | US-04 | AC-04.2 | ADR-005 |
| 21 | Find by exact key | 04 | needs-browser, search, edge | US-04 | AC-04.3 | ADR-005 |
| 22 | Empty state on no match | 04 | needs-browser, search, error | US-04 | AC-04.4 | ADR-005 |
| 23 | Slash typed into search inserted literally | 04 | needs-browser, search, guard, edge | US-04 | AC-04.5 | ADR-002/005 |
| 24 | `Esc` leaves search, restores board | 04 | needs-browser, search, escape | US-04/07 | AC-04.6 | ADR-003/005 |
| 25 | `j` selects + rings first visible card | 05 | needs-browser, selection | US-05 | AC-05.1 | ADR-004 |
| 26 | `j`/`k` walk in on-screen order | 05 | needs-browser, selection, kb-items-collision | US-05 | AC-05.1 | ADR-004/008 |
| 27 | Below-the-fold selection scrolls into view | 05 | needs-browser, selection, edge | US-05 | AC-05.3 | ADR-004 |
| 28 | `k` at first card stays put | 05 | needs-browser, selection, edge | US-05 | AC-05.4 | ADR-004 |
| 29 | Drag selected card leaves ring on the KEY | 05 | needs-browser, selection, drag-coexistence, edge | US-05 | AC-05.8 | ADR-004 |
| 30 | a11y: aria-activedescendant once board focused | 05 | needs-browser, a11y | US-05 | AC-05.7 | ADR-006 |
| 31 | `Enter` opens the selected issue | 05 | needs-browser, open | US-06 | AC-06.1 | ADR-005 |
| 32 | `Enter` with nothing selected does nothing | 05 | needs-browser, open, edge | US-06 | AC-06.2 | ADR-005 |
| 33 | `Enter` in a form submits, no card behind | 05 | needs-browser, open, guard | US-06 | AC-06.3 | ADR-002 |
| 34 | Closing opened issue leaves selection intact | 05 | needs-browser, open, selection | US-06/07 | AC-06.4, AC-07.3 | ADR-004 |
| 35 | **Enter from search opens the same modal as a click** | 05 | needs-browser, open, one-open-path, critical | US-06 | AC-06.5 | ADR-005 |
| 36 | Enter no-op for a found off-board-state issue | 05 | needs-browser, open, edge, named-edge | US-06 | ADR-005 edge | ADR-005 |
| 37 | Shortcuts survive an htmx swap | 05 | needs-browser, property, htmx-swap | X | AC-X.5 | ADR-004 |
| 38 | `#kb-items` gone from the source tree (+ trap B) | 05 | **grep-litmus** (no needs-browser), real-io | US-05 | AC-05.6 | ADR-008 |
| 39 | Manual: real IME + real screen reader | — | manual | US-02/05 | honest limits | ADR-007 |

(39 rows incl. the `@manual`; 38 automated + 1 manual.)

## Coverage & error-path ratio

- Per US: US-01 ×5 (+lane-probe) · US-02 ×6 · US-03 ×3(+2 shared Esc) · US-04 ×6 · US-05 ×7 · US-06 ×6 ·
  US-07 ×6 (shared). Every US-01..07 covered; every slice 01..05 covered.
- **Error / edge / guard / critical scenarios**: #4, #7, #8, #9, #10, #11, #15, #17, #18, #21, #22, #23,
  #27, #28, #29, #32, #33, #36 = **18 / 38 ≈ 47%** (>= 40% target).
- Extras required by the task: `@needs-browser` lane infra (#1, D4=yes), `#kb-items` retirement regression
  (#38), ported `@manual` QA-drill (#39).

## `@needs-browser` lane — exact wiring for DELIVER (ADR-007; probe, then refuse, never skip)

The lane does NOT exist yet. DELIVER executes, in order:

1. **Dependency.** Add `fantoccini` (MIT/Apache-2.0) to `[workspace.dependencies]` (`Cargo.toml:102-109`)
   and to `foundry-acceptance`'s `[dependencies]`. It MUST clear `deny.toml` (`cargo deny check`,
   `xtask/src/main.rs:176`).
2. **`BrowserHarness`.** `support/browser_harness.rs`: `InProcHarness` (UNCHANGED — it already binds
   `127.0.0.1:0` + `axum::serve`, `base_url()` is a real origin) + a `fantoccini::Client` pointed at
   `base_url()`. One chromedriver **process** per lane, one **session** per scenario, `--headless=new`,
   **fixed window size** (deterministic `scrollIntoView`, #27). Store the client on a new `FoundryWorld`
   field (`Option<fantoccini::Client>`). Waits are CONDITIONS not sleeps: `[data-kb-ready]` before any key;
   `#modal-root [data-modal]` after `c`; `document.activeElement` for focus;
   `.issue-card[aria-selected=true]` for the ring.
3. **acceptance.rs lane selection.** ADD `@needs-browser` to the **default-lane exclusion list**
   (`:245-252`, beside `!has("docker-compose")` / `!has("needs-pgclient")` / `!has("slow")`). CONFIRM it
   stays **included in `all`** (`:180-189` excludes only `manual`/`manual-trigger`/`pending`). Do NOT
   exclude it from `all` — a browser-less green gate rebuilds the exact bug this feature closes.
4. **xtask preflight.** In `run_ci`, add a chromedriver+browser preflight mirroring `pg_dump_at_least_16()`
   (`xtask/src/main.rs:335-358`): probe `chromedriver --version` and the browser, parse majors, assert they
   match; on failure print a per-OS hint (`brew install --cask chromedriver` /
   `apt-get install -y chromium-driver`) and exit non-zero. Never `#[ignore]`, never soft-skip.
5. **xtask env-tuple fix (the trap).** `run_steps` currently injects `FOUNDRY_ACCEPTANCE_TAGS` by **label
   substring** (`xtask/src/main.rs:250-257`: `if label.contains("foundry-acceptance") { … "all" }`). A
   SECOND acceptance step cannot be distinguished by label. Change the step tuple to carry its own env,
   e.g. `(&str, Vec<&str>, Vec<(&str, &str)>)`, so the `@needs-browser` step gets its own env explicitly
   and `run_smoke` stays a strict, drift-proof subset.
6. **Register the module** — already done by DISTILL (`lib.rs` steps mod + `acceptance.rs` force-link
   `use … keyboard_shortcut_bindings as _keyboard_shortcut_bindings;`). Without this the `inventory::submit!`
   steps silently vanish.
7. **The startup probe that matters most** (`harness.rs:401-406`): the harness emits `Secure` on the
   session cookie over plain HTTP; reqwest ignores it, a real browser may not. At lane start: sign in,
   navigate, assert STILL signed in — so a Secure-cookie substrate change fails as ONE diagnostic, not as
   every scenario mysteriously failing at sign-in (scenario #1 pins this).

## `#kb-items` retirement — the DELIVER checkpoint (ADR-008, AC-05.6)

Scenario #38 (`@grep-litmus`, NOT `@needs-browser`) is the doneness litmus. DELIVER implements it as a
source-tree grep over `crates/` (or a `cargo xtask check-arch` litmus): **`kb-items` / `kb_items` returns
ZERO hits**. It ALSO guards **trap B** — `projects.rs:1110`'s
`let visible = html.split(r#"id="kb-items""#).next().unwrap();` must be repointed at the full HTML
(`&html`), or `each_issue_lands_in_exactly_its_state_column` passes VACUOUSLY once the carrier is gone
(the exact green-while-absent pattern this whole feature exists to end). Trap A: `projects.rs:1037-1075`
is EDITED (delete only carrier assertions, rename the fn), not deleted. `issue_key_string` STAYS (second
caller at `:912`). Full 13-site map: ADR-008.

## Harness boundary (Architecture of Reference)

| Port class | This feature | Test treatment |
|---|---|---|
| Driving (keyboard -> DOM; shipped GET routes) | the seven keys via `keyboard.js`; `/keyboard-help`, `…/issues/new`, `…/search?q=` (all shipped) | REAL browser (fantoccini -> chromedriver) against REAL `InProcHarness::base_url()` — one app-construction path, so lane and port-to-port suite cannot diverge |
| Driven internal (store) | issues/projects for the board + search fragments | REAL testcontainers Postgres (inherited, `@real-io`) |
| Driven external / non-deterministic | none in scope (selection never reaches the server, BR-5) | n/a |
| Build/test substrate | Chrome + version-matched chromedriver | Host prerequisite + xtask preflight + startup probe (ADR-007), NOT a consumer-driven contract |

## Pre-DELIVER RED classification

Every scenario is `@pending`; the scaffold bodies `panic!` with `__SCAFFOLD__` (assertion-class = RED, not
BROKEN). The scaffold imports only `FoundryWorld` + `cucumber::{given,when,then}` and references no
fantoccini/BrowserHarness seam, so `cargo test -p foundry-acceptance --no-run` stays green and the `all`
lane stays green until DELIVER unskips. DELIVER's RED phase per slice: build the lane (once, slice 01),
remove `@pending` on that slice's scenarios, watch them fail for the RIGHT reason (missing `keyboard.js`
behaviour — not a setup/compile error), implement to GREEN.

## Scaffold wiring decision (STATED per task)

**Choice: the module IS wired** (`lib.rs` steps mod + `acceptance.rs` force-link), matching the
recipient-notification-preferences precedent the task named. Rationale: the scaffold compiles standalone
(no fantoccini — bodies `panic!`), and every scenario is `@pending`, excluded from EVERY lane, so wiring
cannot break `cargo test` or turn the `all` lane red. This gives DELIVER a registered module + the
`__SCAFFOLD__` marker + the pending idiom to extend in place, rather than a dangling unreferenced file.
The scaffold registers a REPRESENTATIVE starter subset of steps (not the full ~90-phrase inventory) to
avoid broad regexes that could collide when DELIVER introduces concrete per-slice steps; the remaining
phrases are inert while `@pending`.

## Scenarios that could NOT be made red-on-`main` (defects to flag) — NONE beyond the sanctioned D15 case

- The US-02 `@paired-assertion` guard (#7) is DELIBERATELY not reds-on-`main` — it is revert-reds-it by
  design (D15). This is sanctioned, documented in the Gherkin and here, and is NOT a defect.
- The `#kb-items` `@grep-litmus` (#38) reds on `main` (the carrier is present today) — correct.
- All 36 remaining browser scenarios red on `main` because `keyboard.js` does not exist. No scenario was
  found that a port-to-port test could satisfy on `main`; none had to be dropped as untestable.
