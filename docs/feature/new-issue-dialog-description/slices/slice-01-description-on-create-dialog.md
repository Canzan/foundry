# Slice 01 — Description on the new-issue dialog (web)

**Goal**: press `c` → dialog shows Title **and** Description → Create → the card lands in Backlog with the
description persisted.

**Story**: US-01.

**IN scope**
- Templates: `partials/new_issue_modal.html` + `new_issue_modal_page.html` each gain
  `<label>Description <textarea name="description">…</textarea></label>`, mirroring `issue_edit_modal.html:7`.
- View models: `NewIssueModal` + `NewIssueModalPage` (`views.rs:79`, `:99`) gain a `description: String` so a
  failed submit can echo the typed value back (AC-01.6 / ODD-4).
- Form: `CreateIssueForm` (`issues.rs:62`) gains `#[serde(default)] description: String` (mirror
  `EditIssueForm`, `issues.rs:269`).
- Service: `issues::create_issue` + the `Services::create_issue` facade (`services/src/lib.rs:57`) gain a
  `description: &str` param; trim/normalize as edit does.
- Store: `insert_issue_with_outbox` (`store/src/lib.rs:1359`) gains a `description: &str` param and writes
  `description_md` in the existing tx — no new query, no migration.
- Tests: store insert/persistence + isolation; the US-01 acceptance scenarios; the change-history
  "no event on create" invariant.

**OUT of scope**
- The API surface (slice 02); the length bound (slice 03); markdown preview; priority/assignee/labels;
  a change-history "created" event kind.

**Learning hypothesis**: disproves **"create cleanly mirrors edit — thread one param through the shipped
pipe"** if (a) the service signature can't take the param without disturbing the API call-site's rule-parity,
(b) the OOB card swap or the error-fragment re-render can't round-trip the typed description, or (c)
`insert_issue_with_outbox`'s tx shape resists the extra column. Confirms it if the only diff is a field at
each layer plus its tests.

**Acceptance**: `discuss/acceptance-criteria.md` US-01 + the store scenarios + the cross-feature
change-history scenarios.

**Seams**: `issue_edit_modal.html:7` (the field to mirror); `new_issue_modal.html`;
`new_issue_modal_page.html`; `keyboard.rs` (`GET …/issues/new`, unchanged route); `CreateIssueForm` /
`submit_create` (`issues.rs:62`/`:70`); `services::create_issue` (`services/src/issues.rs:49`);
`insert_issue_with_outbox` (`store/src/lib.rs:1359`); `update_issue_details_with_outbox` (`:1816`, the
reference shape).

**Watch items**
- Every existing `create_issue` / `insert_issue_with_outbox` call-site must stay behaviorally identical with
  an empty description — including `create_issue_handler`, which passes empty until slice 02.
- The no-JS full-page form is a *second* template; a green htmx lane says nothing about it (AC-01.7).

**Dependencies**: none — DESIGN should resolve ODD-4 (echo-back field) before dispatch.
**Effort**: ~1 day. **Reference class**: `board-new-issue` (whose "near-zero backend" estimate was revised at
DELIVER — this slice is scoped as full-stack from the start).
