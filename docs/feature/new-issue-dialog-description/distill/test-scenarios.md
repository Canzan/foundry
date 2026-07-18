# DISTILL Test Scenarios — new-issue-dialog-description

> Acceptance design (DISTILL). SSOT: `crates/foundry-acceptance/tests/features/new-issue-dialog-description.feature`.
> All `@pending` (excluded from every lane by `acceptance.rs` `filter_run`); DELIVER un-@pends per slice.

## Configuration
- **test_type**: core feature — web create adapter + JSON API adapter + shared service validation + store
  integration.
- **framework**: cucumber-rs; glue in DELIVER at `steps/feature_new_issue_dialog_description.rs`, registered +
  force-linked. **Reuse** the board-new-issue Background steps (identical phrases: workspace/member/project/
  signed-in), the `us_08`/board create helpers, the issue-edit-dialog open/save helpers, and the
  `feature_a_programmatic` machine-credential + API-write helpers.
- **integration**: real Postgres (testcontainers) + HTTP via reqwest + `scraper`; API via the JSON surface.
  `@real-io` on every scenario.
- **HARNESS BOUNDARY**: HTTP-level + direct store assertions, NOT a JS browser. Automated: the modal now
  carries a `description` textarea; the create endpoint persists it; OOB Backlog card; no-JS fallback; API
  parity; the length bound on create AND edit; the change-history coherence invariants. The live
  click→type→Create→card interaction and the field's look are **browser-dogfooded**.
- **ERROR-VISIBILITY BOUNDARY (load-bearing)**: htmx 2.0.4 does not swap 4xx, so validation errors are
  preserved-but-invisible in the browser. Scenarios assert the **HTTP response** (400/422 + fragment/body),
  never a visible DOM message. Typed-input survival is a dogfood item, not an HTTP assertion. This is the
  DESIGN caveat (`upstream-changes.md` §Discovered) made concrete.

## Scenario catalog

### US-01 — web create with description (slice 01)
| # | Scenario | AC | Drives |
|---|----------|----|--------|
| S1 | Dialog offers a Description field | AC-01.1 | GET modal; assert `textarea[name=description]` present + empty |
| S2 | Create with description persists + OOB card | AC-01.2/.3 | POST title+desc (htmx); store `description_md` == typed; OOB backlog card `GEN-1` |
| S3 | Description round-trips to edit dialog | AC-01.4 | create, then GET edit dialog; assert description field pre-filled |
| S4 | Description optional (empty ⇒ "") | AC-01.5 | POST title, empty desc; store `description_md` == ""; card rendered |
| S5 | Empty title rejected, no row (desc typed) | AC-01.6 | POST empty title + desc (htmx); 400 "Title is required" fragment; **no issue created** |
| S6 | No-JS fallback carries field + files desc | AC-01.7 | GET full-page form has textarea; plain POST title+desc → 303; store has desc |
| S7 | Foreign project dialog refused | AC-01.8 | GET new-issue dialog for a foreign project path; uniform not-found |

### US-02 — API create with description (slice 02)
| # | Scenario | AC | Drives |
|---|----------|----|--------|
| S8 | API create with description | AC-02.1/.2 | POST JSON {title,description}; 201 + read-back returns description |
| S9 | API create omitting description | AC-02.1 | POST JSON {title}; 201; `description_md` == "" (back-compat) |

### US-03 — description bound on create AND edit (slice 03)
| # | Scenario | AC | Drives |
|---|----------|----|--------|
| S10 | Over-long on create refused, no row | AC-03.2 | POST 262145-char desc (htmx); "Description is too long" fragment; no row |
| S11 | Over-long on edit refused, issue untouched | AC-03.3 | save edit with 262145-char desc; refused; title+desc **both unchanged** (was a 500) |
| S12 | Exactly at the bound accepted | AC-03.4 | POST 262144-char desc; created; store has 262144 chars |
| S13 | Counted in chars not bytes | AC-03.5 | POST 262144 multi-byte chars; created |
| S14 | API refuses over-long by same rule | AC-02.4 | POST JSON 262145-char desc; **422**; reason matches the browser rule |

### Cross-feature — issue-change-history (slice 01)
| # | Scenario | Invariant | Drives |
|---|----------|-----------|--------|
| S15 | Create with desc emits no event | ODD-5 "start empty" | create with desc; timeline for `GEN-1` empty |
| S16 | First edit reports created desc as old_value | change-history records changes | create desc "First", edit → "Second"; event old="First" new="Second" |

### Store-integration (foundry-store)
| Scenario | Assertion |
|----------|-----------|
| `insert_issue_with_outbox` persists a supplied description | new row `description_md` == supplied value |
| empty description writes "" (byte-identical to today) | every existing caller passing `""` is behaviorally unchanged |
| workspace isolation unchanged by the new param | a create is scoped to the acting workspace exactly as before |

## Port-to-port coverage
- **Driving port (web)**: `GET …/issues/new` (dialog markup, S1/S6), `POST …/issues` (S2/S4/S5/S10/S12/S13),
  `GET …/issues/{n}/edit` (round-trip S3), `POST …/issues/{n}/edit` (S11).
- **Driving port (API)**: `POST /api/v1/…/issues` (S8/S9/S14).
- **Driven port (store)**: `issues.description_md` via `insert_issue_with_outbox` + `update_issue_details`;
  `issue_change_events` timeline (S15/S16).
- No scenario reaches past a driving port into an internal component (hexagonal boundary honored).

## Browser-dogfood checklist (not automated)
1. Press `c` (or click **New issue**) → the dialog shows Title AND Description; Description is empty + editable.
2. Type a title + a multi-line description → **Create** → the card lands in Backlog; reopen it → the
   description is there.
3. Submit with an empty title but a typed description → the create is refused (no card appears) AND the
   dialog still shows the typed description (htmx did not swap the 400). *(This is the input-survival check
   that HTTP cannot see.)*
4. Note for the record: the "Title is required" / "Description is too long" **message is not shown** in the
   browser — expected under the current htmx config; tracked by the deferred error-visibility bugfix.

## Graceful degradation
- DESIGN present (ADR-001/002, architecture.md) → every port maps to a designed seam. Wave-decision
  reconciliation **PASS**: ODD-1..4 ratified; the DISCUSS "unbounded"/"modal destroyed" premises were
  corrected in DESIGN and are reflected here (bound = 262144; S5 asserts HTTP + no-row, survival is a dogfood
  item). No migration; no new component.
