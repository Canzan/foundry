# Requirements: navigation-bar-linear-ui

## Summary

Introduce a **shared global left-sidebar navigation** (Linear-style) into the web tier's shared layout (`crates/foundry-app/templates/base.html`), consolidating today's scattered per-page navigation into one consistent surface across authenticated app pages. Pre-auth / utility pages stay chrome-free.

## Current-state (grounding facts)

- Web tier: `crates/foundry-app`, server-rendered askama/jinja templates (`{% extends %}` / `{% block %}`), HTMX + Alpine.
- Single hashed CSS file `crates/foundry-app/static/css/foundry.4c43c2a8.css` (~360 lines; only a `.brand` class exists — no nav/sidebar styling yet).
- `base.html` shared layout has ONLY `<head>` + `<body>{% block content %}{% endblock %}` — no global nav today.
- 21 templates extend `base.html`.
- Navigation is scattered ad-hoc: `board.html` has an inline `<header>` + `<nav class="board-actions">` (Change report link, New issue button); `dashboard_root.html` has a "Quick actions" list (Invite a member → `/workspace/invites`, Machine tokens → `/admin/tokens`, Keyboard shortcuts → `/keyboard-help`, Instance admin → `/admin/instance/workspaces` gated by `is_instance_admin`, and a CSRF-protected sign-out form `POST /sign-out` with hidden `_csrf = {{ csrf }}`).
- Session/template context provides: `display_name`, `workspace_name`, `is_instance_admin`, `csrf`.

## Functional Requirements

### FR-1 — Shared sidebar on authenticated pages
A persistent left-sidebar rail is rendered on authenticated app pages via the shared layout. Layout regions: **top** = brand + current workspace identity; **middle** = primary destinations; **footer** (pinned) = user/account menu. Page content is offset to the right of the rail.

### FR-2 — Scope: authenticated app pages only
The rail appears on: dashboard (`/`), project board, issue detail, report, token pages, invite pages. It is **absent** on pre-auth / utility pages: `signin`, `forgot`, `forgot_sent`, `bootstrap_dashboard`, `bootstrap_claim`, `bootstrap_invite`, `invite_accept`, `invalid_page`, `payload_too_large`, `events_signin_required`. Those remain chrome-free.

### FR-3 — Primary navigation items (lean, Linear-minimal)
Exactly two primary items: **Home / Dashboard** (`/`) and **Projects / Board**. Each item is an icon + label. No other items are promoted into the primary nav.

### FR-4 — Active-state highlighting
The nav item matching the current route family is visually highlighted (accent-tinted background + higher-contrast label) and carries `aria-current="page"`. Home is active for `/`; Projects is active for board/report/issue routes. Exactly one primary item is active on any authenticated page; never zero, never two. Driven by a server-provided `active_section` value.

### FR-5 — User / account menu (sidebar footer)
A footer anchor shows the current **workspace name** (`workspace_name`) and the **signed-in user display name** (`display_name`). Opening it reveals:
- **Keyboard shortcuts** → `/keyboard-help`.
- **Sign out** → reuse the existing CSRF-protected `POST /sign-out` form with hidden `_csrf = {{ csrf }}`.
- **Instance admin** → `/admin/instance/workspaces`, shown **only** when `is_instance_admin` is true.

### FR-6 — Instance admin gating
The Instance admin item is **absent from rendered HTML** (not merely visually hidden) for non-admins, reusing the existing `is_instance_admin` flag. This prevents a 403-on-click trap.

### FR-7 — Shared context availability
Every authenticated page that renders the rail must receive `display_name`, `workspace_name`, `is_instance_admin`, `csrf`, and `active_section` in its template context. (Today several authed templates — board, issue, report, token, invite — do not all pass these; this is the primary integration work.)

## Non-Functional Requirements

### NFR-1 — Accessibility
The rail is a semantic navigation landmark (`<nav>`). The active item carries `aria-current="page"`. All nav items and the user menu are reachable and operable by keyboard, with visible focus states. Target conformance: WCAG 2.1 AA for the nav component (keyboard operability, focus visibility, name/role/value).

### NFR-2 — Visual quality ("looks like Linear", testable)
- Persistent rail width ~220–260px, quiet neutral surface distinct from content, subtle right border/separator.
- Compact nav items (icon + label), tight-but-generous spacing, rounded hover background.
- Clear active/selected state (accent-tinted background + higher-contrast label).
- User/account block pinned to the sidebar footer.

### NFR-3 — No regression to pre-auth flows
Adding the rail must not alter the rendered output of excluded pre-auth/util pages (they remain chrome-free and unchanged).

### NFR-4 — Consistency with existing patterns
Sign-out reuses the existing `POST /sign-out` endpoint and `_csrf` field name. Admin gating reuses the existing `is_instance_admin` flag. No new auth/CSRF mechanisms are introduced.

## Business Rules

- **BR-1**: Only authenticated sessions with a workspace context render the rail.
- **BR-2**: The Instance admin entry is visible only to instance administrators.
- **BR-3**: Sign-out is always a CSRF-protected POST; never a GET link.
- **BR-4 (Decision #5)**: Invites (`/workspace/invites`) and Machine tokens (`/admin/tokens`) are **deliberately NOT** promoted into the global nav or user menu. They remain reachable via the dashboard's existing "Quick actions" list. These dashboard links **must not be deleted** — nothing is orphaned.

## Explicit Scoping Decisions

- **Decision #5 (recorded)**: Invites and Machine tokens stay in the dashboard Quick actions, not in the rail. DESIGN/DELIVER must not remove those dashboard links while consolidating navigation.

## Deferred (explicitly out of scope for this lightweight pass)

- **Responsive / mobile collapsed drawer** for the rail (note as a future consideration).
- **Promoting Invites / Machine tokens** into the global nav or user menu later.
- **Workspace / team switcher** inside the identity block.

## Open Question (for DESIGN wave)

- **What does the "Projects / Board" primary nav item link to?** No dedicated projects-index route exists today; boards live at `/team/{slug}/project/{slug}`. DESIGN must decide: create a projects-index route, deep-link to a default/last project, or reuse the dashboard "Your projects" section. This affects FR-3, FR-4 (active-state route family), and the Projects link in the shared-artifacts registry.

## Risks

| Risk | Category | Probability | Impact | Mitigation |
|------|----------|-------------|--------|------------|
| Authed pages missing `display_name`/`workspace_name`/`csrf`/`active_section` in context | Technical | High | High | US-04 threads context into all authed handlers; walking skeleton (US-01) limits blast radius to dashboard+board first. |
| "Projects" link target undecided | Project | Medium | Medium | Flagged as DESIGN open question; walking skeleton can ship Home-only active behavior if unresolved. |
| Instance admin item shown to non-admin → 403 trap | Technical | Low | Medium | FR-6: absent-from-HTML gating reusing existing flag; covered by US-03. |
| Accidental deletion of dashboard Invites/Tokens links | Project | Medium | Medium | BR-4 + US-05 regression guard scenario. |
