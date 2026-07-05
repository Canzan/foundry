# Slice 01 — Edit issue title + description (dialog)

**Goal**: click an issue card → edit dialog (title + description, pre-filled) → Save → card updates in place.

**Story**: US-01.

**IN scope**
- Backend (net-new, mirror the shipped patterns): `Store::update_issue_details_with_outbox(prefix, number,
  title, description_md, actor_id)` (mirror `update_issue_state_with_outbox`); `issue_service::edit_issue_details`
  (mirror `change_issue_state`: resolve_member_project authz → validate title → update); handlers
  `show_edit_form` (GET edit dialog fragment, pre-filled + CSRF) + `submit_edit` (save → card OOB-replace +
  close); an `IssueEditModal` view; 2 routes.
- Frontend: `issue_card` gains `hx-get …/issues/{n}/edit` → `#modal-root` (reuse board-new-issue modal +
  styling); the edit dialog form (title input + description textarea, pre-filled) `hx-post`/`hx-patch` →
  OOB-replace the `[data-issue-key]` card + empty `#modal-root`. Keep the plain-form no-JS fallback.
- Store test (isolation + persistence) + the acceptance scenarios.

**OUT of scope**
- state/priority/assignee/labels editing; markdown preview; concurrency-conflict UX; realtime broadcast
  (unless DESIGN opts to emit the outbox event).

**Learning hypothesis**: disproves "issue edit cleanly mirrors change_issue_state + the comment inline-edit
flow, reusing the board modal + OOB card replace" if the save-swap or concurrency needs new machinery.

**Acceptance**: `acceptance-criteria.md` US-01 + the store scenarios.

**Seams**: `render_issue_card`/`issue_card.html`; `#modal-root` + `.modal`; `comments::show_edit_form`/
`submit_edit_comment`; `update_issue_state_with_outbox`; `change_issue_state`; issue schema bounds.

**Dependencies**: DESIGN resolves ODD-1..4 first. **Effort**: ~1 day (net-new backend, well-patterned).
