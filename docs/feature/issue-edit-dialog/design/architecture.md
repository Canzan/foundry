# Architecture — issue-edit-dialog (v1: title + description)

Design for the DISCUSS requirements (`../discuss/`). Ratified ODD-1..4 (2026-07-05, "approve all four"):
GET+POST `/issues/{n}/edit`; OOB card-replace + dialog close; last-write-wins; no realtime/outbox in v1.

## Shape (all mirror shipped patterns; no migration)

```
 Board card (issue_card.html)                       #modal-root (from board-new-issue)
   hx-get …/issues/{n}/edit  ───────────────────►  GET  show_edit_form  ──► IssueEditModal fragment
                                                        (pre-filled title + description + _csrf)
   [ Save ] hx-post …/issues/{n}/edit ────────────►  POST submit_edit
                                                        ├─ edit_issue_details (service)
                                                        │    resolve_member_project (authz, ADR-002/003)
                                                        │    → validate title (1–256, non-empty)
                                                        │    → Store::update_issue_details (lookup by
                                                        │       key_prefix+number → UPDATE title,
                                                        │       description_md, updated_at)   [no outbox — ODD-4]
                                                        └─ on Ok: return the updated card as an OOB
                                                           outerHTML swap keyed on data-issue-key
                                                           (primary body empty → #modal-root clears → dialog closes)
```

## Components (net-new unless noted)

### Store — `crates/foundry-store/src/lib.rs`
- **`update_issue_details(project_key_prefix, issue_number, title, description_md, actor_id) -> Result<Option<()>, IssueInsertError>`**
  — mirrors `update_issue_state_with_outbox` (`:1273`) MINUS the outbox emit (ODD-4). Lookup the issue by
  `p.key_prefix = $1 AND i.number = $2`; `Ok(None)` if absent; else `UPDATE issues SET title=$, description_md=$,
  updated_at=now() WHERE id=$`. Last-write-wins (no `updated_at` guard — ODD-3).
- **`issue_edit_view(project_key_prefix, issue_number) -> Result<Option<IssueEditRow>, _>`** (read for pre-fill)
  — SELECT title, description_md for the issue (scoped upstream by the resolved project). If an equivalent
  issue-fetch already exists behind `show_issue`, reuse it instead of adding a new one (DELIVER checks).

### Service — `crates/foundry-services/src/issues.rs`
- **`edit_issue_details(store, principal, team_slug, project_slug, number, title, description_md) -> Result<BoardIssue, ServiceError>`**
  — mirror `change_issue_state` (`:85`): `resolve_member_project` (authz + non-enumerable NotFound) → validate
  `title` (non-empty, ≤256) → `store.update_issue_details(...)` → return the `BoardIssue { key, number, title, state }`
  so the handler can re-render the card. (A read for the pre-fill also goes through a `resolve_member_project`-
  gated service fn for tenancy.)

### Web — `crates/foundry-app/src/issues.rs` (+ `views.rs`, `lib.rs`, templates)
- **`GET  …/issues/{n}/edit` → `show_edit_form`** — resolve acting workspace (ADR-002); fetch the issue's
  title+description via the gated read; render `IssueEditModal` (title input pre-filled, description textarea
  pre-filled, hidden `_csrf`, `hx-post` to the save URL, `hx-target="#modal-root"`, `method=post` fallback).
  Foreign/missing issue → `resource_not_found_page()` (byte-identical, ADR-003).
- **`POST …/issues/{n}/edit` → `submit_edit`** — CSRF (middleware); call `edit_issue_details`; on `Ok`, if
  htmx: return `200` with the updated card as `<article class="issue-card" data-issue-key="{key}"
  hx-swap-oob="outerHTML">…{new title}…</article>` (primary body empty → `#modal-root` clears → dialog
  closes). If not htmx: `303` → the board. On `Validation` (empty/oversized title): `bad_request_fragment`
  rendered in the dialog (mirror `submit_create`). `Forbidden`/`NotFound`: the shipped pages.
- **`views::IssueEditModal { action, csrf, key, title, description }`** — a `.modal`/`.modal-dialog` fragment
  (reuse the board-new-issue styling); mirrors `NewIssueModal` plus the description textarea + pre-filled values.
- **`partials/issue_card.html`** — the `<article class="issue-card" data-issue-key="{{ key }}">` gains
  `hx-get="…/issues/{{ number }}/edit"` + `hx-target="#modal-root"` + `hx-swap="innerHTML"` (and `style="cursor:pointer"`
  or a role, minor). `render_issue_card` (`issues.rs:280`) gains the same attributes so the OOB-replaced card
  stays clickable. NOTE: the board card render must expose the issue `number` (the card currently carries only
  `key` + `title`) — a small `BoardIssue`/card-view field addition, analogous to the board-slug surfacing in
  board-new-issue.

## Cross-cutting

- **Tenancy (ADR-002/003)**: both endpoints scope by the resolved acting workspace; a foreign issue → uniform
  `resource_not_found_page`. No new `check-arch` LAYER-1e line (no `*_in_workspace(` request-parsed id).
- **CSRF**: save is under `csrf_middleware` + double-submit `_csrf` in the form.
- **No-JS fallback**: the dialog form keeps `method="post" action="…/edit"`; a plain submit hits `submit_edit`
  (non-htmx branch → 303 board).
- **No migration**; last-write-wins; no outbox emit (v1).

## Slice plan

One slice (`slice-01-edit-title-description`). DELIVER may internally order it store→service→handlers→template
→acceptance, but it ships as one vertical (click→edit→save→card updates).
