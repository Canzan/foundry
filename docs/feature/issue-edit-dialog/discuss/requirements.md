# Requirements — issue-edit-dialog

## Context

On a project board, an issue **card** (`partials/issue_card.html`:
`<article class="issue-card" data-issue-key="{{ card.key }}">`) is **static** — clicking it does nothing.
There is a `GET …/issues/{n}` detail page (`comments::show_issue` → `issue.html`) but it shows only
**attachments + comments**, not the issue's own fields, and it is a full page, not a dialog. The only
editable issue field today is **state** (`change_issue_state` → `update_issue_state_with_outbox`); there is
**no** update path for `title` or `description_md`.

This feature makes an issue card **clickable → open an edit DIALOG** (Linear-style modal) to edit the issue's
**title + description** (v1), save it, and see the board card update in place. It builds on the modal
infrastructure shipped by `board-new-issue` (`#modal-root` + `.modal`/`.modal-dialog` styling) and mirrors the
app's existing inline-edit (comments) + outbox-update (state) patterns.

Scope for v1 (per user, 2026-07-05): **title + description only**. State/priority/assignee editing are
deferred increments (the schema supports them: `state`, `priority`, `assignee_id`).

## JTBD (anchor job)

> **When** I see an issue on the board whose title or details are wrong or incomplete, **I want** to click it,
> fix the text in a focused dialog, and save, **so I can** keep the board accurate without leaving it or
> hunting for an edit page.

- **Functional**: correct an issue's title/description. **Emotional**: the board feels editable/trustworthy.
- **Social**: a shared board whose issues stay accurate for the team.

## Personas

| ID | Persona | Cares about |
|----|---------|-------------|
| P1 | A workspace member on the board | Click a card → edit its title/description → save → the card reflects the change. |

## Scope (v1 — title + description)

- **In scope**: clicking an issue card opens an edit dialog (modal) pre-filled with the issue's current
  `title` + `description_md`; editing + Save persists both; the board card updates in place; the dialog closes.
  Empty title is rejected in the dialog (mirrors create). Non-JS fallback: the dialog form is a plain POST
  that saves and returns to the board.
- **Out of scope** (deferred increments): editing **state / priority / assignee** in the dialog (schema
  supports them — separate slices); labels & project (no schema); the attachments/comments page (unchanged);
  rich-text/markdown-preview editing (description is a plain textarea in v1); optimistic-concurrency conflict
  UX (DESIGN decides last-write-wins vs `updated_at` guard); realtime broadcast of edits to other viewers
  (DESIGN decides whether to emit the outbox event now).

## Brownfield grounding (seams — REUSE / MIRROR; DESIGN owns the exact shapes)

| Seam | Location | Role |
|------|----------|------|
| Issue card (make clickable) | `partials/issue_card.html`; `issues.rs::render_issue_card` (`:280`) | Gains `hx-get …/issues/{n}/edit` → `#modal-root`. On save, replaced in place via an OOB `outerHTML` swap keyed on `data-issue-key`. |
| Modal container + styling (REUSE) | `board.html` `#modal-root`; `.modal`/`.modal-dialog` in the vendored stylesheet | The edit dialog swaps into the SAME container the new-issue modal uses; same overlay styling. |
| Inline-edit pattern (MIRROR) | `comments.rs::show_edit_form` (`:225`) + `submit_edit_comment` (`:292`); `partials/comment_edit_form.html` | The GET-edit-form → PATCH-save → swap-updated-fragment flow to mirror (as a modal, not inline). |
| State-update store method (MIRROR) | `Store::update_issue_state_with_outbox` (`lib.rs:1273`) | Model for the NEW `update_issue_details_with_outbox(prefix, number, title, description_md, actor_id)` (UPDATE title, description_md, updated_at; + outbox — DESIGN decides). |
| Issue-edit service (MIRROR) | `issue_service::change_issue_state` (`services/issues.rs:85`) | Model for the NEW `edit_issue_details(...)`: `resolve_member_project` authz → validate title (1–256) → update. |
| Issue schema | `issues.title` (1–256 CHECK), `issues.description_md` (≤262144 CHECK) | The two v1-editable fields + their validation bounds. |
| CSRF + tenancy | `csrf_middleware`; `resolve_member_project` / acting workspace (ADR-002/003) | The edit GET + save POST sit under CSRF + session; authz + non-enumerable 404 mirror `show_issue`/`submit_create`. |

## Constraints

- **Net-new backend** (unlike the last two features): a store update method, a service method, and two
  handlers (GET edit dialog, POST/PATCH save) + a view. No migration (fields exist). DESIGN owns the exact
  endpoint verbs/shapes, the OOB-replace mechanics, concurrency, and whether to emit the outbox event.
- **Tenancy (ADR-002/003)**: scope by the resolved acting workspace; a foreign issue → uniform
  `resource_not_found_page`, never an enumeration oracle.
- **CSRF** on the save; **no-JS fallback** preserved (plain POST saves + redirects to the board).
- **Reuse the board-new-issue modal infra** (`#modal-root` + styling) — do not add a second modal system.
- **Title validation** identical to create (1–256, non-empty → "Title is required").

## Open decisions (for DESIGN)

- ODD-1: endpoint verbs — `GET …/issues/{n}/edit` + `POST` vs `PATCH …/issues/{n}`.
- ODD-2: save response — OOB `outerHTML` replace of the card (keyed on `data-issue-key`) + empty `#modal-root`
  to close, vs a full board refresh.
- ODD-3: concurrency — last-write-wins vs `updated_at` optimistic guard.
- ODD-4: emit the realtime outbox event on edit now, or defer to the realtime increment.
