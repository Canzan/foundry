# DISCUSS Decisions — new-issue-dialog-description

## Key Decisions

- **[D1] API keeps rule-parity with the web dialog** (user-ratified 2026-07-17): `CreateIssueRequest` gains an
  **optional** `description`. `create_issue_handler` (`foundry-api/src/lib.rs:378`) already calls the same
  `services::create_issue` as the browser and documents the invariant as *"the SAME path the browser handler
  uses — identical validation/authz/outbox"* (NFR-WEB-API-CON-02). The service param is being added
  regardless; leaving the API unable to send it would knowingly break a stated invariant to save one struct
  field. Omitting the field stays byte-compatible for existing clients. (ODD-2 resolves the serde shape.)
- **[D2] One description bound, applied to create AND edit** (user-ratified 2026-07-17): a
  `DESCRIPTION_MAX_LEN` lands in the shared service validation and guards both `create_issue` and
  `edit_issue_details`. **This corrects a shipped gap** — `description_md` is currently unbounded on the edit
  path while `TITLE_MAX_LEN = 256` guards the title (`issues.rs:60`, `:188`). Bounding create only would give
  users two dialogs with two rules for one field. See `## Upstream Changes`. (ODD-1 sets the value.)
- **[D3] Description is optional**: empty is valid and must behave exactly as today (`description_md = ""`).
  Title-only capture is the dominant path and gains no friction. The anxiety force in J1 is otherwise real.
- **[D4] Both create surfaces carry the field**: `partials/new_issue_modal.html` (htmx) **and**
  `new_issue_modal_page.html` (the no-JS full-page fallback). The no-JS contract has held since
  `board-new-issue` D4; a fallback that silently drops the description would be worse than no field.
- **[D5] This is a full-stack change, NOT template-only — stated up front.** The field is absent at all four
  layers: template → `CreateIssueForm` (`issues.rs:62`) → `services::create_issue` → `insert_issue_with_outbox`
  (`store/lib.rs:1359`). **Precedent**: `board-new-issue` D5 was written as "near-zero backend change" and had
  to be **REVISED at DELIVER** when a view-model field turned out to be required. Verified in-tree here to
  avoid repeating that. No migration — `description_md` exists; latest stays `0014`.
- **[D6] Mirror, don't invent**: the field's markup copies `issue_edit_modal.html:7` verbatim in shape; the
  service/store threading mirrors `edit_issue_details` / `update_issue_details_with_outbox`. Reuse over
  reconstruction.
- **[D7] Repo convention**: legacy multi-file nWave layout (per `[[foundry-nwave-old-multifile-convention]]`);
  no SSOT/`docs/product/`, no feature-delta, no migration. The skill's SSOT migration gate is intentionally
  not honored. Trunk-based: commit to `main`, no PR.

## Open Design Decisions (for DESIGN)

| # | Question | Proposal |
|---|----------|----------|
| ODD-1 | What is `DESCRIPTION_MAX_LEN`? | Propose **16384** chars (64× the title bound; comfortably above a long issue body, far below a paste-bomb). Counted with `chars().count()` to match the title rule. |
| ODD-2 | Serde shape for the API field | `#[serde(default)] description: String` mirrors `EditIssueForm` (`issues.rs:269`). Confirm `null` vs absent vs `""` all normalize to empty. |
| ODD-3 | Error copy + fragment for over-long description | Mirror `bad_request_fragment("Title is required")`; propose code `description_too_long`. |
| ODD-4 | Does the re-rendered error form need the description echoed back? | AC-01.6 requires it. `NewIssueModal` likely needs a `description` field to round-trip the failed input. |

## Requirements Summary

- **Primary need (J1)**: let a member write the description at capture time, in the dialog `c` opens, instead
  of create → find card → reopen → edit.
- **Secondary need (J2)**: keep the API able to send what the UI sends (rule-parity).
- **Integrity precondition**: bound the description on both paths (D2).
- **Walking skeleton**: n/a — brownfield; the create path, modal, CSRF, OOB card swap and `description_md`
  column all ship. This threads one value through an existing pipe.
- **Feature type**: user-facing, brownfield, changing existing functionality.

## Constraints Established

- Tenancy, CSRF, and the no-JS fallback all preserved; no migration (latest stays `0014`).
- Rule-parity (NFR-WEB-API-CON-02): one shared service; identical validation copy across web and API.
- US-R07 (templates extend `base.html`).
- Existing API clients omitting `description` must be unaffected.

## Cross-Feature Verification

- **issue-change-history (ODD-5, "start empty")**: v1 records field *changes*, not creation, and
  `insert_issue_with_outbox` is marked **REUSE (no change)** in that feature's architecture. A description
  supplied at create time is therefore correctly **not** a timeline event, and the first later edit reports it
  as `old_value`. Checked against `docs/feature/issue-change-history/design/architecture.md` — **no conflict**.
  Covered by explicit scenarios so the invariant is asserted, not assumed.

## Scope Assessment: PASS

Right-sized. 3 stories → **3 slices**, one bounded vertical (create path) plus one shared validation helper.
Oversized signals checked: >10 stories ✗ | >3 bounded contexts ✗ (issues only) | WS needs >5 integration
points ✗ | effort >2 weeks ✗ (~1.5 days) | independent shippable outcomes — **2 fire** (web create, API
create), which is exactly why they are separate slices rather than a split of the feature. No split needed.

## Upstream Changes

- **`issue-edit-dialog` — validation bounds**: that wave's DoR asserted "validation bounds" among its NFR
  constraints; verification found the description unbounded at every layer while the title is bounded at 256.
  D2 closes it via the shared service validation (US-03). Recorded in `requirements.md` `## Changed
  Assumptions`; no issue-edit-dialog document was modified.
- **`board-new-issue` — none.** Its D1-D4 (modal container, close-on-success via OOB + empty target, error
  inside the modal, no-JS fallback) all hold unchanged; this feature adds a field inside that shipped frame.
