# Requirements — board-new-issue

## Context

On a project board (`GET /team/{team}/project/{project}` → `projects::show_board`) the **"New issue"**
button renders but is **inert**: `board.html:5` emits `<button type="button" data-action="new-issue">` with
no `hx-*`, no `@click`, no script — and there is NO app-owned JS or Alpine directive binding it (only vendored
htmx + alpine load). Confirmed live: clicking the button and pressing `c` fire **no request** and open no
modal. The empty-state's "press `c` to file the first one" is aspirational — the key was never bound either.

The **backend is complete and tested**: `GET …/issues/new` returns the new-issue modal fragment
(`keyboard::show_new_issue_modal` → `partials/new_issue_modal.html`), and `POST …/issues`
(`issues::submit_create`) creates the issue and, **on an htmx request, returns an out-of-band card fragment**
that appends itself to the Backlog column (`hx-swap-oob="beforeend:[data-column='backlog']"`). Only the
**client-side wiring** to open the modal from the button and submit it via htmx is missing.

Scope for this feature (per user, 2026-07-04): **button-only** — make the "New issue" button work. The
keyboard shortcut layer (`c`/`Esc`/focus) is explicitly OUT (deferred).

## JTBD (anchor job)

> **When** I'm looking at an empty (or full) project board, **I want** to click "New issue", type a title,
> and see the issue appear, **so I can** capture work the moment I think of it without leaving the board.

- **Functional**: create an issue from the board. **Emotional**: the board feels alive/responsive.
- **Social**: a shared board that actually captures the team's work.

## Personas

| ID | Persona | Cares about |
|----|---------|-------------|
| P1 | A workspace member on a team's board | Clicking "New issue" opens a form and filing it drops a card into Backlog. |

## Scope (v1 — button-only)

- **In scope**: the "New issue" button opens the existing modal (htmx GET), the modal's title form submits
  (htmx POST), the returned card lands in **Backlog**, and the modal dismisses. The empty-state hint updates
  naturally once a card exists.
- **Out of scope** (deferred): the `c` keyboard shortcut + `Esc`-to-close + focus management (the US-12
  interaction layer — its own feature `board-keyboard-interaction`); inline client validation styling of the
  error fragment beyond what the backend already returns; drag-between-columns; creating directly into a
  non-Backlog column.

## Brownfield grounding (shipped seams — reuse, do NOT reinvent)

| Seam | Location | Reuse |
|------|----------|-------|
| The inert button | `crates/foundry-app/templates/board.html:5` | Gets `hx-get` (modal endpoint) + `hx-target`/`hx-swap` (a modal container). |
| Modal endpoint | `GET …/issues/new` → `keyboard::show_new_issue_modal` → `views::NewIssueModal` / `partials/new_issue_modal.html` | Returns the modal fragment verbatim; no change needed to the endpoint. |
| Modal fragment form | `partials/new_issue_modal.html` | `<form action="{{ action }}">` with `title` + hidden `_csrf`. Gets `hx-post="{{ action }}"` + target so submit is an htmx request. |
| Create POST | `POST …/issues` → `issues::submit_create` (`issues.rs:47`) | On htmx: returns `render_issue_card_with_column_marker` — an **OOB** card that appends to `[data-column='backlog']` (`issues.rs:293`). On validation error: `bad_request_fragment("Title is required")`. Unchanged. |
| CSRF | double-submit `_csrf` in the modal form + `csrf_middleware` | The modal fragment must carry the `_csrf` token (the endpoint already provides it via `NewIssueModal.csrf`). |
| Board container | `board.html` `.board` / `[data-column='…']` sections | The OOB swap targets `[data-column='backlog']` — already present. A NEW modal-container element is the only markup addition. |

## Constraints

- **Near-zero backend change** (REVISED — see `## Changed Assumptions`): endpoints, service, OOB card, CSRF all
  shipped. The ONLY `src/` change is exposing the team + project **slugs** on the `BoardPage` view-model so the
  button's `hx-get` can address the modal endpoint (2 fields + populate + 1 test fix; no new logic, no migration).
- **CSRF preserved** — the modal form keeps the hidden `_csrf`; an htmx POST carries it.
- **Tenancy** — `submit_create` already scopes by the resolved acting workspace (foreign/missing project →
  uniform `resource_not_found_page`); unchanged.
- **Graceful no-JS** — if htmx is unavailable, the modal form is a plain POST that still creates the issue and
  redirects to the board (the shipped non-htmx branch). Wiring must not break that fallback.
- **US-R07** — all markup stays in templates extending `base.html`; no handler emits inline HTML.

## Open decisions (resolved in wave-decisions.md)

- OD-1: where the modal fragment is swapped (new container vs replace the button) — D1.
- OD-2: how the modal closes after a successful create (OOB card + empty the modal target) — D2.
- OD-3: error path — the `bad_request_fragment` must render inside the open modal, not replace the board — D3.

## Changed Assumptions

**Original (DISCUSS/DISTILL)**: "This is template/markup wiring only … No backend change" — the seam table
assumed the board template could address `…/issues/new` with slugs already in scope.

**Discovered (DELIVER, 2026-07-05)**: `BoardPage` (`crates/foundry-app/src/views.rs:198`) exposes
`team_name`, `project_name`, `key_prefix`, `columns`, `kb_items` — **no slugs**. The board template renders no
`/team/.../project/...` URL to copy, and no Askama slugify filter exists, so a robust `hx-get` cannot be built
template-only (relative URLs resolve wrong on the no-trailing-slash board path; `name|lower` breaks on
multi-word names).

**New assumption + rationale**: the slice adds `team_slug` + `project_slug` (`String`) to `BoardPage`,
populated in `projects.rs::build_board_page` from the existing `ProjectRow.slug` and `slugify(team_name)`, with
one unit-test call-site update. This surfaces already-present data to the template — no new logic, no
migration, no service/route change. User-authorized 2026-07-05. AC-01.7 is revised accordingly.
