# User Stories — board-new-issue

## US-01 — File an issue from the board's "New issue" button

**As a** workspace member viewing a project board (P1)
**I want** the "New issue" button to open a title form and file the issue
**so that** I can capture work from the board without knowing a keyboard shortcut or leaving the page.

### Elevator Pitch
Before: the "New issue" button on the board does nothing when clicked (no request, no modal).
After: click **New issue** → a modal opens → type a title → **Create** → the new card appears in **Backlog**
and the modal closes, no full-page reload.
Decision enabled: I decide to file the work I just thought of, immediately, and see it captured.

### Acceptance Criteria
- AC-01.1: Clicking "New issue" issues a `GET …/issues/new` and renders the modal (title field + Create).
- AC-01.2: Submitting the modal with a title issues an htmx `POST …/issues` carrying the `_csrf` token.
- AC-01.3: On success the returned card is appended to the **Backlog** column (via the shipped OOB swap) and
  the modal is dismissed — **no full-page navigation**.
- AC-01.4: The new card shows the canonical issue key (`GEN-1`, …) and the title.
- AC-01.5: Submitting an **empty** title renders the shipped "Title is required" error **inside the open
  modal** (the board is not replaced; no card is created).
- AC-01.6: **No-JS fallback preserved** — with htmx disabled, the modal form is a plain POST that still
  creates the issue and lands the board showing the new card (the shipped redirect branch).
- AC-01.7: Near-zero backend change — the modal endpoint, create POST, OOB card, and CSRF are unchanged; the
  ONLY `src/` edit is exposing `team_slug` + `project_slug` on the `BoardPage` view-model (populated from the
  existing `ProjectRow.slug` + `slugify(team_name)`) so the button can address the modal endpoint (D5). No new
  logic, no migration.
