# DISTILL — Acceptance Self-Review: keyboard-shortcut-bindings

> Quinn (nw-acceptance-designer), DISTILL wave. This is the AUTHOR'S self-review against the six
> critique dimensions + the house self-review checklist. A separate `nw-acceptance-designer-reviewer`
> (Sentinel) runs AFTER this as the independent gate; its output is PR-ephemeral, not committed here.

## Six critique dimensions

### 1. Business-language purity (Pillar 1) — PASS
Scenario titles and Given/When/Then read in domain language: "presses", "the keyboard shortcut list
appears as an overlay", "the new-issue modal opens", "the results list shows the empty state". Selectors,
`data-*` attributes, `aria-*`, fantoccini, `client.execute` live only in comments and step bodies (to be
written by DELIVER), never in the Gherkin surface. One deliberate near-technical term — `AUTH-2` — is a
domain identifier (an issue key the user reads on a card), not jargon.

### 2. Observable-outcome / hexagonal boundary — PASS
Every automated scenario enters through a DRIVING surface (a real keypress in a real browser, or the
sidebar link with JS off) and asserts a USER-VISIBLE outcome (overlay present, modal focused, ring on a
card, `aria-activedescendant` set, empty state shown). No scenario reaches into internal client state;
even selection is asserted via its observable consequence (`aria-selected` / the same modal a click
produces), per ADR-004. The one non-browser scenario (#38) is an explicit SOURCE-TREE litmus, tagged
`@grep-litmus` and documented as such — not smuggled in as a browser assertion.

### 3. Error / edge coverage — PASS (47% >= 40%)
18 / 38 automated scenarios are error/edge/guard/critical: no-project no-op, copy-chord non-activation,
IME suppression, Shift-not-a-suppressor, empty search state, bounded selection, drag coherence, Enter
no-ops (empty selection + off-board-state), layered Esc, the paired guard. The guard cliff (US-02) drives
the architecture rather than trailing it (slice 02 before slice 03, D11).

### 4. Reds-on-`main` integrity (NFR-1 / D9) — PASS
36 browser scenarios red on `main` because `keyboard.js` does not exist. #38 reds on `main` because the
`#kb-items` carrier is still present. The single sanctioned exception is the US-02 `@paired-assertion`
(#7): revert-reds-it by design (D15), kept as ONE scenario with both halves, with a Gherkin warning and a
test-scenarios.md warning NOT to split. No scenario passes vacuously on `main`.

### 5. Chained-narrative / one-open-path fidelity (Pillar 2) — PASS
Slice narratives chain: slice 03's Esc reuses slice 01's overlay; slice 05's Enter reuses slice 05's
selection; the layered-Esc scenario chains `c` -> `?` -> `Esc` -> `Esc`. The one-open-path scenario (#35)
proves keyboard and pointer converge on the board card's own `hx-get` (ADR-005), and the htmx-swap property
(#37) proves the key survives a re-render (ADR-004). No copy-pasted fixture setup: preconditions build on
prior steps' outcomes.

### 6. Harness-boundary correctness (Pillar 3) — PASS
Production composition root reused AS-IS: `InProcHarness` (real axum + testcontainers Postgres) + a real
browser. One app-construction path — the lane and the port-to-port suite cannot diverge (upstream-changes
§1). No in-memory doubles (nothing to double: selection is client-only, the routes are shipped). The two
honest limits (IME simulated, clipboard non-assertable) are named at their scenarios, not hidden.

## House self-review checklist

- [x] WS strategy declared (walking-skeleton.md — REAL driving + driven-internal; the lane-probe skeleton).
- [x] WS/lane scenarios tagged correctly (`@needs-browser @real-io`; `@grep-litmus` and `@manual` are NOT `@needs-browser`).
- [x] Every NEW driven surface exercised REAL — the three shipped routes via the real browser; no synthetic doubles.
- [x] In-memory doubles: N/A (documented — nothing to double).
- [x] Mandate 7: the imported production seam (the `@needs-browser` lane) is scaffolded RED (`@pending` panic scaffold, `__SCAFFOLD__` marker).
- [x] Driving adapter: every keypress + the no-JS link + the shipped GET routes are exercised via their real protocol (a real browser), not by calling a service fn.
- [x] Scaffold includes `__SCAFFOLD__` marker (`keyboard_shortcut_bindings.rs`).
- [x] Scaffold bodies raise assertion-class panic (Red-Gate = RED, not BROKEN); no `NotImplementedError`-equivalent.
- [x] Tests are RED (not BROKEN) when unskipped — scaffold compiles against `FoundryWorld` + cucumber only.
- [x] `@real-io @adapter-integration` equivalent: the lane-probe + all browser scenarios drive real I/O (real browser, real socket, real Postgres).
- [x] Timing: waits are CONDITIONS not sleeps (documented for DELIVER; `[data-kb-ready]`, `document.activeElement`, ring selector) — no flaky sleep budget.
- [x] `@when` boundary: N/A to Rust cucumber (no pytest capsys); the `world`-captured-body idiom is the repo pattern DELIVER follows.

## Mandate 9 / 11 (layered PBT discipline)

All `@needs-browser` scenarios are **layer 3+** (real browser, real adapter). Per Mandate 9/11 they are
**example-only** — no `@given`/proptest generators. The three `@property` scenarios (#3 bound==advertised,
#6 no-JS, #37 htmx-swap) are EXAMPLE-BASED invariant litmuses in the house sense, NOT PBT-generated. This
is correct for the layer and is stated in the feature file and test-scenarios.md. No Tier B state-machine
is warranted: the observable is "did the key produce the outcome", there is no rich domain input space to
model, and selection state is a single string (ADR-004).

## Definition of Done (DISTILL -> DELIVER gate)

- [x] All acceptance scenarios written; scaffold compiles (`@pending` panic bodies).
- [x] Test pyramid: acceptance `.feature` + planned unit locations noted (guard predicate is a pure fn, ADR-002 — DELIVER can unit-test `isTextEntry` without a browser).
- [ ] Independent peer review (Sentinel) — PENDING (runs after this self-review).
- [x] Tests run in CI/CD — the lane joins `cargo xtask ci`'s `all` (DELIVER wires; instructions in test-scenarios.md).
- [x] Story demonstrable to stakeholders — the lane-probe skeleton is the demo.
- [x] Reconciliation HARD GATE passed (0 contradictions; D2 amendment is reconciled/ratified).
- [x] Target language detected — Rust (`cargo`, cucumber-rs); `[lang-mode] rust`.
- [x] State-delta port: N/A — this is the Rust cucumber-rs harness, not the Python state-delta pilot; the repo's world-captured-observable idiom is the layer-3 universe guard.
- [x] Mandate 8 (universe/observable): assertions are port-exposed observables (overlay present, modal focused, aria-*, exit of grep), never internal client fields.
- [x] Pillar 1/2/3 verified above.

## Open items handed to DELIVER

1. Build the `@needs-browser` lane FIRST (slice 01) — fantoccini dep, `BrowserHarness`, acceptance.rs
   exclusion, xtask preflight + env-tuple fix. Exact steps in test-scenarios.md §"@needs-browser lane".
2. Execute the ADR-008 13-site `#kb-items` retirement in slice 05, minding trap A (edit+rename) and trap B
   (repoint `visible` at full HTML). The `@grep-litmus` (#38) is the doneness checkpoint.
3. Retire the us-12-keyboard-nav.feature `@manual` drill + its stale `:18-23` module doc when slice 05
   lands 7/7 (ADR-007 §5).
4. Fill the scaffold's representative step subset + add the remaining concrete per-slice Given/When/Then
   phrases as each slice is unskipped.
5. Put "Tab to the board, then j/k" into the help overlay copy (ADR-006 KPI-4 obligation) — asserted by #30.

## Self-review verdict

**CONDITIONALLY APPROVED — ready for independent review.** All six dimensions pass; the one non-reds-on-main
scenario is the sanctioned D15 paired assertion; both DISTILL-open AC notes are resolved; the lane wiring is
specified precisely enough for DELIVER to execute without rediscovery. Blocking item before DELIVER GREEN:
the independent `nw-acceptance-designer-reviewer` pass.
