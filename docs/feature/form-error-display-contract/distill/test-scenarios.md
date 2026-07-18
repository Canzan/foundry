# DISTILL Test Scenarios — form-error-display-contract

> Acceptance design (DISTILL). SSOT: `crates/foundry-acceptance/tests/features/form-error-display-contract.feature`.
> All `@pending`; DELIVER un-@pends per slice. **Browser-lane only** (ADR-002).

## Configuration
- **test_type**: defect remediation (escalated from `/nw-bugfix`) — a client-side display contract.
- **framework**: cucumber-rs; DELIVER glue at `steps/feature_form_error_display.rs`, registered + force-linked.
  Reuse the shipped `@needs-browser` harness (`support/browser_harness.rs`, `world.rs` fantoccini client) and
  the keyboard-shortcut-bindings navigation/`press`/DOM-read helpers where phrasing matches.
- **integration**: real Postgres (testcontainers) + **fantoccini + chromedriver** driving a real browser
  against `InProcHarness::base_url()`. `@needs-browser` (IN the `all` lane, excluded from the fast default).
- **HARNESS BOUNDARY (the whole point — ADR-002)**: the fix is client-side JS, so the HTTP lane is BLIND to it
  (byte-identical 400/422 before and after; only the DOM changes). Every scenario drives a REAL browser and
  asserts the error is **present and visible in the rendered DOM**. The shipped HTTP-lane oracles (assert the
  400/422 + fragment body) are KEPT in their own features and are NOT touched here — they guard the server
  contract; these add the DOM assertion that never existed (closes RCA Root Cause B).

## Scenario catalog

### Slice 01 — the contract + the oracle, proven on issue create (walking skeleton)
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S1 | Lane can observe a rejected submit end to end | handler loaded + slot exists; empty-title submit → "Title is required" visible, dialog open | `@lane-probe @walking_skeleton` |
| S2 | Invalid create shows the reason, creates nothing | error visible in dialog + dialog open + **no card added** | `@error @contract` |
| S3 | Fix + resubmit succeeds, no reload | after the error, typing a title + resubmit → dialog closes, card in Backlog, no navigation (proves `_csrf` preserved by slot-only swap) | `@error @edge` |
| S4 | A successful create is unaffected | valid submit → closes, card appears, **no error shown anywhere** (blast-radius guard: handler fires only on 4xx) | `@error @scoped` |

### Slice 02 — fan out to the remaining htmx forms
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S5 | Invalid issue edit shows the reason | clear title + Save → "Title is required" visible in the edit dialog, dialog open, card unchanged | `@error @contract` |
| S6 | Invalid comment edit shows the reason inline | empty body + Save → error visible next to the comment, original text kept | `@error @contract` |

### Slice 03 — the edges (DELIVER may defer)
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S7 | Rejected drag reverts AND says why | refused move → card returns to origin **and** a message is visible (today: silent snap-back) | `@edge @drag` |
| S8 | Invalid new comment shows the reason, not a blank page | rejected comment → error visible on the issue page, page not replaced by a bare fragment | `@edge @comment-create` |

## Port-to-port coverage
- **Driving port**: the real browser DOM (the user's actual surface) — the only port that exercises the
  htmx + `form-errors.js` + form-markup integration. Each covered form (issue create/edit, comment edit, and
  the drag/comment-create edges) has a DOM-visibility scenario.
- **Driven port**: the store — S2 asserts no row created; S3 asserts the card lands; S5 asserts the issue is
  unchanged. Real Postgres.
- No scenario asserts the HTTP body here (that's the shipped HTTP lane's job, retained) — these assert the DOM.

## Falsification (a passing scenario must be able to fail)
- **S1/S2 MUST be shown RED against a build WITHOUT `form-errors.js`** (or without the `[data-error-slot]`):
  the error stays invisible → the "visible inside the dialog" assertion fails. This is the direct reproduction
  of the defect and the proof the oracle can see it — the thing the HTTP lane could never do.
- **S4 MUST be shown RED against a handler that fires on 2xx too** (over-broad guard): a successful create
  would wrongly surface an error / mis-swap → S4 fails. Guards the blast radius.
- **S3 MUST be shown RED against a full-`#modal-root` replace on error** (dropping the form + `_csrf`): the
  resubmit would fail CSRF → no card → S3 fails. Proves the slot-only swap is load-bearing.

## Browser-lane operational notes (carried)
Per `[[foundry-browser-lane-fantoccini]]`: chromedriver version-match preflight (in `cargo xtask ci`),
testcontainer leakage → `PoolTimedOut`, and the no-JS/timing flakes. DELIVER budgets for them and uses bounded
`wait().for_element` conditions (never sleeps), matching the keyboard-shortcut-bindings lane discipline.

## Graceful degradation
DESIGN present (ADR-001/002) → every scenario maps to a designed seam (`form-errors.js`, the per-form slot,
the retained server fragment). Wave-decision reconciliation **PASS**: the mechanism (option d), the oracle
(browser DOM), and the edges are all ratified; ODD-1..4 carry proposals for DELIVER. No server change, no
migration.
