# DISTILL Test Scenarios — board-new-issue

> Acceptance design (DISTILL). Source Gherkin SSOT:
> `crates/foundry-acceptance/tests/features/board-new-issue.feature`. Every scenario `@pending` (excluded from
> all lanes via `acceptance.rs filter_run`); DELIVER un-@pends as it wires the templates + step glue.

## Configuration

- **test_type**: web adapter (front-end wiring) over a shipped backend.
- **framework**: cucumber-rs (`tests/features/*.feature`; step glue authored in DELIVER under
  `crates/foundry-acceptance/src/steps/feature_board_new_issue.rs`, reusing `us_07`/`us_08` board + sign-in +
  create helpers).
- **integration approach**: real Postgres (testcontainers) + the HTTP surface via reqwest + `scraper` DOM
  assertions. Tag `@real-io`.
- **layer**: LAYER-3.
- **HARNESS BOUNDARY (key)**: the suite is HTTP-level, **not a JS browser** — it cannot execute htmx. So the
  *interactive* behavior (click → modal swaps in → submit → card appends → modal closes, no reload) is NOT
  automatable here. The automated scenarios instead pin: the **wiring attributes** (S1, S2), the **shipped
  endpoint contracts** the wiring depends on (S3 OOB card, S4 error fragment), and the **no-JS fallback** end
  to end (S5). The live interaction is verified by **browser dogfood** (claude-in-chrome) — the same split
  `us-12-keyboard-nav` used for its "press c" flow (manual checklist + automated endpoints).

## Scenario catalog

| # | Scenario | AC | What it drives | RED state |
|---|----------|----|----|-----------|
| S1 | New-issue button wired to open the modal | AC-01.1 | GET board; `scraper` asserts the button's `hx-get` → `…/issues/new` + `hx-target` + a modal container exists | @pending — button is inert today (`data-action` only) |
| S2 | Modal form wired to submit via htmx | AC-01.2 | GET `…/issues/new`; assert the form's `hx-post` → action + retained `method=post` + hidden `_csrf` | @pending — modal form has no `hx-post` today |
| S3 | htmx create returns an OOB Backlog card | AC-01.3/.4 | POST `…/issues` (HX-Request) title+`_csrf`; assert OOB fragment `beforeend:[data-column='backlog']` + key + title | reuses shipped `issues.rs:293` contract (regression pin) |
| S4 | Empty title → error fragment, not a card | AC-01.5 | POST `…/issues` (HX-Request) empty title; assert 400 "Title is required" bare fragment, no card/board | reuses shipped `bad_request_fragment` contract |
| S5 | No-JS fallback files the issue | AC-01.6 | POST `…/issues` (NO HX-Request) title+`_csrf`; assert 303 → board; GET board shows the card | @pending — proves the plain-form contract stays intact after wiring |

## Browser-dogfood checklist (not automated — verified live at DELIVER)

1. Open the "Sandbox" board; click **New issue** → the modal appears (no reload).
2. Type a title, click **Create** → the card appears in **Backlog** and the modal closes, no reload.
3. Submit an empty title → "Title is required" shows **inside the modal**; the board is untouched.

## Graceful degradation log

- **DESIGN absent**: WARN not block — the reuse-only seam table (`requirements.md`) + D1–D5 substitute; every
  scenario's port maps to a verified-present seam.
- **State-delta port**: none (Rust suite convention); LAYER-3 assertions are traditional over the DOM + status
  + DB rows.
- **Wave-decision reconciliation**: PASS — D2 (close via OOB + empty target) and D4 (no-JS fallback) map
  directly onto S3/S5; 0 contradictions.
