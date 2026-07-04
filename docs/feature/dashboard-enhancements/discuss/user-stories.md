# User Stories — dashboard-enhancements

All stories trace to the anchor job (see `requirements.md` § JTBD). Each maps to one elephant-carpaccio
slice (`story-map.md`). ACs are embedded here and expanded to Given/When/Then in `acceptance-criteria.md`.

---

## US-01 — Personalized greeting (identity + workspace)

**As a** signed-in user (P1/P2/P3)
**I want** the dashboard to greet me by name and name my current workspace
**so that** I can confirm at a glance who I am and where I am before I start work.

### Elevator Pitch
Before: the dashboard says only "You are signed in. Welcome back." — no name, no workspace.
After: visit `/` → sees `Welcome back, {display_name}` and `Workspace: {workspace_name}`.
Decision enabled: I confirm I'm acting in the right workspace before creating/editing anything.

### Acceptance Criteria
- AC-01.1: The dashboard shows the signed-in user's `display_name` and the acting workspace's `name`.
- AC-01.2: Both values are loaded via ONE store query scoped by the SESSION `user_id`/`workspace_id`.
- AC-01.3: Values are HTML-escaped (Askama auto-escaping; a `display_name` with `<`/`&` renders inert).
- AC-01.4: If the query errors, the page still renders with a neutral fallback greeting (no 500) — D1.
- AC-01.5: `<h1>Foundry</h1>` remains present (US-R04).

---

## US-02 — Sign out

**As a** signed-in user
**I want** a Sign out control on the dashboard
**so that** I can end my session on a shared machine without editing URLs.

### Elevator Pitch
Before: there is no sign-out affordance in the UI; the session can only be ended by clearing cookies.
After: click **Sign out** on `/` → POSTs to `/sign-out` → session cleared → redirected to `/sign-in`.
Decision enabled: I decide to safely hand off / walk away from the machine.

### Acceptance Criteria
- AC-02.1: The dashboard renders a `<form method="post" action="/sign-out">` with a submit button.
- AC-02.2: The form carries a valid double-submit `_csrf` token matching the `foundry_csrf` cookie set on
  the same response (mirrors `admin_tokens::show_index`).
- AC-02.3: Submitting signs the user out (session destroyed) and 303-redirects to `/sign-in`.
- AC-02.4: A forged/absent `_csrf` is refused by `csrf_middleware` (the shipped behaviour, unchanged).
- AC-02.5: After sign-out, `GET /` 303-redirects to `/sign-in` (no longer "signed in").

---

## US-03 — Instance-admin link for super-admins only

**As an** instance super-admin (P3)
**I want** a link to the instance-provisioning surface on my dashboard
**so that** I can reach tenant administration without remembering the URL — **and** as a non-super-admin
(P1/P2) I must never see that link.

### Elevator Pitch
Before: `/admin/instance/workspaces` is reachable only by typing the URL; nothing surfaces it.
After: a super-admin visiting `/` sees an **Instance admin** action; a member/admin sees no such link.
Decision enabled: a super-admin decides to provision/administer tenants from a discoverable entry point.

### Acceptance Criteria
- AC-03.1: When `Store::is_instance_admin(session user_id)` is true, the dashboard shows an "Instance
  admin" link to `/admin/instance/workspaces`.
- AC-03.2: When false, the link is absent from the rendered HTML (not merely hidden with CSS).
- AC-03.3: The check uses the SESSION `user_id` only (no path/query id).
- AC-03.4: The link's presence adds no enumeration oracle beyond the already-gated route (the route itself
  stays non-enumerable for non-super-admins).

---

## US-04 — Promote dashboard styles into the vendored stylesheet

**As a** maintainer
**I want** the dashboard's styles to live in the vendored stylesheet, not inline in the template
**so that** styling is consistent with the rest of the app and cache-busts correctly.

> `@refactor` — behaviour-preserving. This story ships alongside a value story in its slice (never alone),
> per the slice-composition rule.

### Elevator Pitch
Before: dashboard styling is an inline `<style>` block in `dashboard_root.html`.
After: the same styles live in `static/css/foundry.<newhash>.css`; `base.html` links the bumped hash; the
rendered dashboard is visually identical.
Decision enabled: (maintainer-facing) future style edits happen in one canonical stylesheet.

### Acceptance Criteria
- AC-04.1: No `<style>` block remains in `dashboard_root.html`; the dashboard classes are defined in the
  vendored CSS file.
- AC-04.2: `base.html` references the new content hash; the old hash no longer appears in the tree.
- AC-04.3: The rendered dashboard is visually equivalent (same layout/classes) before and after.
- AC-04.4: `/static/css/foundry.<newhash>.css` is served 200 by ServeDir; the old filename 404s.

---

## US-05 — Test coverage for the dashboard (retroactive + new)

**As a** maintainer
**I want** the dashboard query and render covered by tests at the repo's bar
**so that** the surface can't silently regress and mutation testing has something to bite.

> `@infrastructure`/test-debt for the already-shipped base (`51ba981`), plus the new behaviours' tests are
> authored test-first WITHIN US-01..US-03's slices. This story tracks the retroactive base coverage.

### Elevator Pitch
Before: `list_projects_for_workspace` and `dashboard_root` shipped in `51ba981` with zero tests.
After: `cargo test -p foundry-store` covers the project-list query (scoping + ordering + empty); an
acceptance scenario drives the signed-in dashboard end-to-end.
Decision enabled: (maintainer-facing) I trust the dashboard is protected before layering more on it.

### Acceptance Criteria
- AC-05.1: A store test asserts `list_projects_for_workspace` returns a workspace's projects, ordered by
  name, and is **isolated** (a second workspace's projects never leak in).
- AC-05.2: A store test asserts the empty case (workspace with no projects → empty vec).
- AC-05.3: An acceptance scenario: a signed-in user visiting `/` sees their project(s) and the quick
  actions; drives through to a project board link.
- AC-05.4: The new store query for US-01 (greeting) is covered analogously (scoping + fallback).
