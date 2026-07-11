# Slice 05 — Signed-in per-workspace notification status page

**Goal**: give an account holder a signed-in page showing, per workspace they belong to, whether they are
subscribed or muted → Maria can see at a glance where she stands, own state only.
**Story**: US-05.

**IN scope**
- A signed-in `GET /account/notifications` page registered beside `/account/password`
  (`crates/foundry-app/src/lib.rs:415-418`), under session + CSRF layers.
- Resolve the member's own identity from the **session** (`SessionUser`, `crates/foundry-app/src/session.rs`) →
  `email_lower` via `find_user_by_email` / the users table (`foundry-store/src/lib.rs:930`); **never** a
  client-supplied email (NFR-6).
- For each workspace the member belongs to (membership lookups `is_team_member` / `is_workspace_admin`,
  `foundry-store/src/lib.rs:1048,1955`): render **Muted** iff `exists_unsubscribe(email_lower, workspace_id)`
  (slice 01 store method), else **Subscribed**.
- Reachable from the nav footer/menu (`crates/foundry-app/src/nav.rs:16,29,35-37`, under `NavSection::Home`).
- WCAG 2.1 AA basics (NFR-8): labelled controls, status conveyed as text (not colour alone), keyboard-navigable.
- Acceptance: accurate per-workspace status; only the member's own workspaces/status; a request naming another
  user's email returns only the session member's scope.

**OUT of scope**: the **resubscribe** action (US-06 — this slice is read-only); muting from the settings page
(the email link is the mute path in v1); the operator suppression metric (US-07); account-less resubscribe
(ODD-6).

**Learning hypothesis**: disproves "the per-workspace subscription state (from the `0014` table) can be surfaced
to an account holder as an own-only, session-scoped status page reusing the shipped session + membership
lookups, with no cross-recipient enumeration" if identity can be steered from the request, if the membership
listing leaks other recipients, or if the status can't be derived cleanly from the single-source table.

**Seams**: authed route neighbour `/account/password` (`lib.rs:415-418`); session `SessionUser`
(`session.rs:64`, `build_session_layer` `:69`); `find_user_by_email` (`foundry-store/src/lib.rs:930`);
membership lookups (`:1048,1955`); `resolve_active_workspace` (`:804`); `exists_unsubscribe` (slice 01); nav
(`nav.rs:16,29,83`).
**Dependencies**: slice 01 (US-01) — the unsubscribe state it reads. DESIGN ODD-7 (multi-workspace
presentation). No new persistence, no new migration.
**Effort**: ~1 day (a read-only authed page + a11y; new user-facing surface).
</content>
