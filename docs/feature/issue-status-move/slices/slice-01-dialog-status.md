# Slice 01 — Status control in the edit dialog

**Goal**: the edit dialog gains a status control; saving a new status moves the card to that column.
**Story**: US-01.

**IN scope**
- Extend `IssueEditModal` + `partials/issue_edit_modal.html` with a status `<select>` (four states, current
  pre-selected).
- `submit_edit` also applies the state (DESIGN ODD-3: fold into the edit update vs call `change_issue_state`).
- Card-relocation mechanic (DESIGN ODD-2): on save, the card leaves its old column + lands in the new (server
  OOB move) + dialog closes. This mechanic is REUSED by slice 02.
- Store/acceptance: the dialog-drives-state path; reuse the shipped `change_issue_state`.

**OUT of scope**: drag-and-drop; reorder; cancelled; priority/assignee.

**Learning hypothesis**: disproves "status-in-dialog is a small extension reusing change_issue_state + a
server-driven column move" if folding state or the OOB move needs new machinery.

**Seams**: `IssueEditModal`/`issue_edit_modal.html`; `submit_edit`; `change_issue_state`; board `[data-column]`
+ `render_issue_card`.
**Dependencies**: DESIGN ODD-2/ODD-3. **Effort**: ~0.5–1 day.
