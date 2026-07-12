# Architecture (grounding note) — notification-preferences-ui

> **Seed grounding**, not a full DESIGN wave. The user chose a lean DISTILL → DELIVER
> pass; this note records the one genuinely-new architectural element (a dedicated
> settings surface) and its reuse-over-reinvent grounding so the acceptance designer
> has concrete port-to-port targets. Open decisions are left for DELIVER.

## Reuse-over-reinvent map (shipped seams)

| Seam | Location | Role here |
|------|----------|-----------|
| Shared sidebar rail + `NavContext` presentation projection | `crates/foundry-app/src/nav.rs`, `templates/partials/sidebar.html` | The **entry point** is a new `<a>` in the footer `sidebar__user` block. `NavContext` already carries `csrf`, identity, and is assembled on every authed page — no new plumbing to make the link appear everywhere. |
| Signed-in notifications page | `crates/foundry-app/src/unsubscribe.rs::show_notifications` (`GET /account/notifications`) | The **data path to reuse**: `workspaces_for_member` + `list_unsubscribed_workspace_ids` → `(name, muted)` rows. The settings surface renders the same rows. |
| Signed-in resubscribe | `unsubscribe.rs::resubscribe_notifications` (`POST /account/notifications/resubscribe`) | Reused unchanged; the pattern (session identity, `workspaces_for_member` membership check, `delete_unsubscribe`, uniform 404) is the **template for the new mute action**. |
| Public unsubscribe write | `unsubscribe.rs::submit_confirm` → `Store::insert_unsubscribe` | The **already-shipped mute write** the new signed-in mute action calls (guarded by a session-scoped membership check, mirroring resubscribe). |
| Notifications templates | `templates/notifications.html`, `views::{NotificationsPage, NotificationRow}` | The list rendering to extend with a per-row mute control. |
| Router | `crates/foundry-app/src/lib.rs::build_router` | Where the new `/account/settings` (+ any signed-in mute POST) routes mount, on the authenticated layer under the shipped `session_layer` + `csrf_middleware`. |

## The one new element: a signed-in settings shell

- **`GET /account/settings`** — an authenticated page (session-gated, uniform 404 when
  signed out, like `show_notifications`) that renders a settings shell whose first
  section is **Notifications**, populated by the existing `show_notifications` data path.
  It assembles the shared rail via `NavContext::home_for` (Home stays the active primary
  item — settings is a footer destination, NFR-3).
- **Sidebar link** — a `Settings` (or `Notifications`) `<a class="sidebar__item">` in the
  footer `sidebar__user` block, alongside `Keyboard shortcuts`.
- **`POST /account/settings/mute`** (or `/account/notifications/mute`) — the new
  signed-in mute action: session identity → `workspaces_for_member` membership check →
  `Store::insert_unsubscribe(email_lower, workspace_id)` → same result page as
  resubscribe. Idempotent (`ON CONFLICT DO NOTHING`), CSRF-checked, non-enumerable.

## Invariants to hold (regression-guarded)
- Navigation-bar `exactly-one-active-primary` invariant (`nav.rs` tests + the
  `navigation-bar-linear-ui` acceptance sweep) — settings must NOT become a third primary.
- Least-privilege / non-enumerability of the notifications handlers (no request-supplied
  email or foreign workspace can steer state).
- Existing `recipient-notification-preferences` acceptance coverage stays green (existing
  `/account/notifications` + `/unsubscribe` behavior preserved).

## Open decisions for DELIVER
- **OD-1** `/account/settings` as canonical shell vs. keeping `/account/notifications`
  canonical and linking to it. Recommendation: `/account/settings` shell; either embed
  the notifications section or 302 `/account/notifications` → `/account/settings#notifications`.
- **OD-2** Sidebar label (`Settings` vs `Notifications`) and exact footer placement.
- **OD-3** Route path for the new mute POST and its result-page reuse (`render_result_page`).
