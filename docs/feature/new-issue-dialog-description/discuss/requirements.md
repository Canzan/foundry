# Requirements — new-issue-dialog-description

## Problem

The new-issue dialog (opened by the `c` shortcut or the board's **New issue** button) collects a **Title
only**. The *edit* dialog, shipped by `issue-edit-dialog`, collects **Title + Description**. So the only way
to give an issue a description is to create it, find the card, and reopen it in a second dialog. The
`issues.description_md` column already exists and edit already writes it — the create path simply never
carries the value.

## Verified current state (in-tree, 2026-07-17)

The gap is **full-stack**, not template-deep. Every layer of the create path drops the description:

| Layer | Create path (today) | Edit path (shipped) |
|-------|--------------------|--------------------|
| Template | `templates/partials/new_issue_modal.html` — Title only | `templates/partials/issue_edit_modal.html:7` — `<label>Description <textarea name="description">` |
| Fallback template | `templates/new_issue_modal_page.html` — Title only | n/a |
| View model | `views.rs:79` `NewIssueModal { project_name, action, csrf }` | `IssueEditModal { …, description }` |
| Form struct | `issues.rs:62` `CreateIssueForm { title, _csrf }` | `issues.rs:267` `EditIssueForm { title, description, state, … }` |
| Service | `foundry-services/src/issues.rs:49` `create_issue(store, principal, team, project, title)` | `edit_issue_details(…, title, description)` |
| Facade | `foundry-services/src/lib.rs:57` `create_issue(…, title)` | — |
| Store | `foundry-store/src/lib.rs:1359` `insert_issue_with_outbox(…, title)` | `update_issue_details_with_outbox(…, description_md)` at `:1816` |

**No migration.** `description_md` exists and is written today by the edit path — latest migration stays
`0014`.

## Two findings that shape scope

**F1 — The create service is shared with the JSON API.** `foundry-api/src/lib.rs:378 create_issue_handler`
calls the same `services::create_issue` as the browser handler. Its doc comment states the invariant
explicitly: *"the SAME path the browser handler uses — identical validation/authz/outbox"* and
*"rule-parity"* / NFR-WEB-API-CON-02. Threading description through the service therefore reaches the API
surface whether or not we expose it there — so exposing it is the coherent choice (D1).

**F2 — `description_md` is unbounded on the shipped edit path.** `TITLE_MAX_LEN = 256` is enforced at
`issues.rs:60` (create) and `:188` (edit); there is **no** corresponding description bound anywhere. The
shipped edit dialog accepts an arbitrarily large description. Adding create without a bound would widen an
existing hole; bounding create only would split one user-visible rule across two dialogs. So the bound lands
in the shared validation and applies to both (D2) — this is an **upstream correction to `issue-edit-dialog`**
(see `## Changed Assumptions`).

## Functional requirements

| # | Requirement | Source |
|---|-------------|--------|
| FR-1 | The new-issue dialog renders a `description` textarea beside the title input, mirroring the edit dialog's markup. | US-01 |
| FR-2 | Description is **optional**; empty is valid and behaves exactly as today. | US-01 / AC-01.5 |
| FR-3 | The description is persisted to `issues.description_md` on create and round-trips to the edit dialog. | US-01 |
| FR-4 | Both create surfaces carry the field: the htmx modal partial and the no-JS full-page fallback. | US-01 / AC-01.7 |
| FR-5 | A typed description survives a title-validation re-render of the form. | US-01 / AC-01.6 |
| FR-6 | `CreateIssueRequest` accepts an optional `description`; omitting it is byte-compatible for existing clients. | US-02 |
| FR-7 | A `DESCRIPTION_MAX_LEN` bound is enforced in shared service validation on **both** create and edit. | US-03 |
| FR-8 | Over-long descriptions are refused with nothing persisted; at-bound is accepted; counting is by `chars()`. | US-03 |

## Non-functional constraints

- **Tenancy**: unchanged — `resolve_member_project` authz on create is untouched.
- **CSRF**: unchanged — the modal keeps its hidden `_csrf` double-submit field.
- **No-JS fallback**: preserved on both create templates (the standing contract since `board-new-issue` D4).
- **Rule-parity (NFR-WEB-API-CON-02)**: web and API share one service; validation copy is identical; the
  returned representation equals a subsequent read.
- **No migration**: `description_md` ships; latest migration remains `0014`.
- **US-R07**: templates extend `base.html`.
- **Reuse over reconstruction**: mirror `issue_edit_modal.html` markup and the `update_issue_details_with_outbox`
  / `edit_issue_details` shapes rather than inventing new ones.

## Dependencies and seams

| Seam | Location | Use |
|------|----------|-----|
| Edit dialog markup | `partials/issue_edit_modal.html:7` | The exact field to mirror |
| New-issue modal | `partials/new_issue_modal.html` | Gains the field |
| No-JS fallback page | `new_issue_modal_page.html` | Gains the field |
| Modal endpoint | `keyboard.rs` — `GET …/issues/new`, backs the `c` shortcut, emits the fragment on `HX-Request` | Unchanged route; view model gains a field |
| Create form / handler | `issues.rs:62` `CreateIssueForm`, `:70` `submit_create` | Gains `description` |
| Create service | `foundry-services/src/issues.rs:49` | Gains param + description validation |
| Store insert | `foundry-store/src/lib.rs:1359` | Gains param; writes `description_md` |
| API create | `foundry-api/src/lib.rs:378` + `CreateIssueRequest` | Gains optional field (D1) |
| Edit validation | `foundry-services/src/issues.rs:188` | Gains the shared description bound (D2) |
| Change history | `issue-change-history` ODD-5 "start empty" | **Consistent** — creation is not a change, so no event; verified, not assumed |

## Out of scope

- Markdown **preview** in the create dialog (edit doesn't have it either — parity means parity).
- Priority / assignee / labels / state selection at create time.
- Backfilling descriptions for existing issues.
- A "created" event kind in the change-history timeline (ODD-5 explicitly defers this; `old_value` is already
  nullable for a future creation-event kind).
- Rich-text or attachment support.

## Changed Assumptions

**Source**: `docs/feature/issue-edit-dialog/discuss/requirements.md` — the issue-edit-dialog wave established
the description field's contract and stated its validation constraint as, per its DoR item 8, *"Tenancy,
CSRF, no-JS fallback, no migration, validation bounds"*.

**Original assumption**: that the edit dialog's shipped **validation bounds** covered the description field
as they cover the title.

**New assumption**: they do not. Verification in-tree found `TITLE_MAX_LEN = 256` enforced at
`foundry-services/src/issues.rs:60` and `:188`, and **no description bound at any layer** — the shipped edit
path accepts an unbounded `description_md`.

**Rationale for the change**: this feature must decide what create does with an over-long description. Every
available answer except "bound both" either widens the existing hole or splits one user-visible rule across
two dialogs that users experience as one surface. The bound therefore lands in the shared service validation
and retroactively closes the edit-path gap (US-03, D2).

**Preservation**: no `issue-edit-dialog` document is modified. This section is the record.
