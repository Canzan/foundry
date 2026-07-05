# Requirements — issue-status-move

## Context

A project board renders four state columns (Backlog / Todo / In-Progress / Done; slugs
`backlog`/`todo`/`in_progress`/`done`), but a card **cannot be moved between them** from the UI. The board was
built anticipating this — `projects.rs:580`: "the other columns stay empty placeholders until drag-and-drop".

The **state-change backend is fully shipped and tested**: `POST …/issues/{n}/state` (`submit_state_change`) →
`issue_service::change_issue_state` → `Store::update_issue_state_with_outbox`; `normalize_state` accepts the
column slugs directly. BUT the endpoint returns a state **chip** (`partials/state_chip.html`), an in-place
indicator — **not** a card that relocates to the new column. And there is **no app-owned JS** (only vendored
htmx + Alpine), so drag-and-drop is genuinely new.

This feature lets a member move an issue between statuses two ways (per user, 2026-07-05):
1. **Direct edit** — a **status control in the edit dialog** (`issue-edit-dialog`) changes the state on save.
2. **Drag and drop** — dragging a card into another column changes its state.

Both reuse the shipped state backend; both need the **card to relocate to its new column** (the shared
mechanic); DnD additionally needs the app's **first client-side JS**.

## JTBD (anchor job)

> **When** an issue's status changes (I start it, finish it, or triage it), **I want** to move its card to the
> right column — by dragging it or picking a status in its dialog — **so I can** keep the board an accurate,
> at-a-glance picture of what's where.

## Personas
| ID | Persona | Cares about |
|----|---------|-------------|
| P1 | A member working the board | Drag a card to another column, OR set its status in the edit dialog — and it sticks. |

## Scope (v1)

- **In scope**:
  - **Slice 1 (dialog status)**: the edit dialog gains a status control (current state pre-selected); saving
    persists the new state and the card **moves to the matching column**.
  - **Slice 2 (drag-and-drop)**: cards are draggable; columns are drop targets; dropping a card into a column
    persists the new state and the card lands there. Uses the app's first client-side JS (approach = DESIGN).
- **Out of scope** (deferred): reordering cards WITHIN a column (only cross-column status moves); the
  `cancelled` state (no column); priority/assignee via drag; multi-select drag; realtime broadcast of moves to
  other viewers (ODD — DESIGN decides whether to emit the shipped outbox event's effect to the SSE consumer);
  touch-drag polish beyond baseline.

## Brownfield grounding (seams — REUSE / MIRROR; DESIGN owns the mechanic)

| Seam | Location | Role |
|------|----------|------|
| State-change endpoint (REUSE) | `POST …/issues/{n}/state` → `submit_state_change` (`issues.rs:126`) → `change_issue_state` (`services/issues.rs:85`) → `update_issue_state_with_outbox` (`lib.rs:1273`) | The persist path for BOTH mechanisms. `normalize_state` (`services/issues.rs:33`) accepts the column slugs. |
| Current return = chip (RETHINK) | `render_state_chip` / `partials/state_chip.html` | Today returns an in-place state chip, not a card move. The card-relocation mechanic (OOB move vs client DOM move) is the DESIGN crux. |
| Board columns + cards | `board.html` `[data-column='{slug}']`; `issue_card.html` `[data-issue-key]`; `render_issue_card` (`issues.rs:280`) | Drop targets + draggable cards; the card carries key/title and (from issue-edit-dialog) `hx-get` edit + `number`. |
| Edit dialog (EXTEND — slice 1) | `issue-edit-dialog`: `IssueEditModal`, `show_edit_form`/`submit_edit`, `partials/issue_edit_modal.html` | Gains a status `<select>`; `submit_edit` also sets state (fold into the edit save vs call change_issue_state — DESIGN). |
| Modal infra (REUSE) | `#modal-root` + `.modal`/`.modal-dialog` styling | The dialog is unchanged structurally; just one more control. |
| Vendored client libs | `static/vendor/{htmx,alpine}.min.js`; NO app JS | DnD needs JS: native HTML5 DnD API + a small app JS file, vs a vendored SortableJS, vs Alpine handlers (DESIGN). |

## Constraints

- **Reuse the shipped `/state` persist path** — do not add a second state-write path.
- **Tenancy/CSRF**: `change_issue_state` already scopes by acting workspace + is CSRF-guarded; both mechanisms
  inherit it. A drop/select for a foreign issue → uniform non-enumerable refusal.
- **No-JS graceful degradation**: the dialog status control works via a plain form POST (no JS); drag-and-drop
  is a progressive enhancement — without JS, the board is unchanged (no drag), and the dialog remains the
  no-JS path to change status. US-R07 (templates extend base.html).
- **No migration** (state column + values exist).
- **First app JS (DnD)** must be a vendored file (project convention: vendored libs under `/static/vendor`, or
  a small app-owned `/static/js`) — DESIGN decides; it must be self-contained + CSP-safe.

## Open decisions (for DESIGN)

- ODD-1: **DnD approach** — native HTML5 Drag-and-Drop API + a small app JS file, vs vendored **SortableJS**, vs
  **Alpine** directives.
- ODD-2: **card-relocation mechanic** — server OOB swap (remove card from old column + append to new) driven by
  the `/state` response, vs client-side DOM move (natural for DnD) with the server just persisting.
- ODD-3: **dialog save + state** — fold state into `submit_edit` (one update) vs the dialog posting to the
  shipped `/state` endpoint separately.
- ODD-4: **realtime** — leave moves local to the actor (v1) vs broadcast via the shipped outbox to other viewers.
