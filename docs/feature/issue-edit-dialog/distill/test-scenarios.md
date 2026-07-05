# DISTILL Test Scenarios — issue-edit-dialog

> Acceptance design (DISTILL). SSOT: `crates/foundry-acceptance/tests/features/issue-edit-dialog.feature`.
> All `@pending` (excluded from every lane); DELIVER un-@pends as it ships slice 01.

## Configuration
- **test_type**: web adapter (net-new edit backend) + store integration.
- **framework**: cucumber-rs; glue in DELIVER at `steps/feature_issue_edit_dialog.rs` (reuse `us_07`/`us_08`
  board + sign-in + issue-seed helpers), registered + force-linked.
- **integration**: real Postgres (testcontainers) + HTTP via reqwest + `scraper`. `@real-io`.
- **HARNESS BOUNDARY**: HTTP-level, not a JS browser. Automated: wiring (card `hx-get`), pre-fill fragment,
  save endpoint contract (OOB card replace + store persistence), validation, tenancy, no-JS fallback. The live
  click→dialog→save→card-updates interaction + the modal look are **browser-dogfooded**.

## Scenario catalog
| # | Scenario | AC | Drives |
|---|----------|----|--------|
| S1 | Card wired to open edit dialog | AC-01.1 | GET board; assert `GEN-1` card `hx-get …/issues/1/edit` + target |
| S2 | Dialog pre-filled with current values | AC-01.1 | GET the edit endpoint; assert title/description pre-filled + `hx-post` + `_csrf` |
| S3 | Save edits issue + replaces card | AC-01.2/.3 | POST save (htmx); assert store has new title/desc + OOB `outerHTML` card keyed on `GEN-1` |
| S4 | Empty title rejected in dialog | AC-01.4 | POST empty title; assert "Title is required" fragment; store unchanged |
| S5 | Foreign issue refused non-enumerably | AC-01.6 | GET edit for a foreign issue path; uniform not-found, no echoed title |
| S6 | No-JS fallback saves | AC-01.8 | POST (no HX-Request); 303 → board; store updated |

### Store-integration (foundry-store)
| Scenario | Assertion |
|----------|-----------|
| `update_issue_details` persists both fields | title + description_md updated, `updated_at` bumped |
| tenant isolation | a foreign-workspace issue is not updated by a scoped call |

## Browser-dogfood checklist (not automated)
1. Click `GEN-1` → the pre-filled edit dialog opens (centered modal).
2. Change the title/description → Save → the board card shows the new title, dialog closes, no reload.
3. Clear the title → Save → "Title is required" in the dialog; board untouched.

## Graceful degradation
- DESIGN present (ADR-001/002) → every port maps to a designed seam. Wave-decision reconciliation PASS
  (ODD-1..4 ratified, reflected in S3/S6). No migration; last-write-wins; no outbox (v1).
