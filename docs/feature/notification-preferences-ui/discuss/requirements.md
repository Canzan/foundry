# Requirements — notification-preferences-ui

> **Seed grounding** (not a full DISCUSS wave). Captured by `/nw-new` to give the
> lean DISTILL → DELIVER pass a solution-neutral problem statement. The requirements
> and per-workspace mute/subscribe *backend* are already SHIPPED by the predecessor
> feature `recipient-notification-preferences`; this feature only makes them
> **reachable and self-service from the signed-in UI**.

## Context

Foundry already ships a full recipient-notification-preferences backend:

- **`GET /account/notifications`** (`crates/foundry-app/src/unsubscribe.rs::show_notifications`)
  — a signed-in page listing every workspace the session user belongs to with a
  `Muted` / `Subscribed` status, derived least-privilege from the caller's own
  `email_lower` opt-out rows.
- **`POST /account/notifications/resubscribe`**
  (`unsubscribe.rs::resubscribe_notifications`) — CSRF-checked signed-in unmute.
- The public signed-link **`GET`/`POST /unsubscribe`** confirm/mutate flow for
  account-less recipients.

**The gap**: nothing in the app's UI links to `/account/notifications`. The shared
Linear-style sidebar (`crates/foundry-app/templates/partials/sidebar.html`) exposes
`Home`, `Board`, `Keyboard shortcuts`, a conditional `Instance admin`, and `Sign out`
— but no path to notification preferences. A signed-in user therefore cannot reach
the shipped page except by an emailed unsubscribe link. Additionally, the signed-in
page today offers **resubscribe only**; there is no signed-in **mute** action.

## Jobs To Be Done

### JOB-1 `manage-my-notifications-from-the-app` — signed-in member

> **When** I am signed in and want to control which workspaces email me, **I want to**
> find and change my notification preferences from inside the app, **so I can** mute or
> resume a workspace's emails without hunting for an old message's unsubscribe link.

- **Push**: preferences are only reachable via an emailed link today — invisible in-app.
- **Pull**: a discoverable, self-service settings entry point in the sidebar.
- **Anxiety**: "Will muting here leak my identity / affect other people?" (No — the
  shipped backend is least-privilege, session-scoped, non-enumerable.)
- **Habit**: users expect account/notification settings behind a sidebar/account menu.

## Scope

### In scope
- **FR-1** A **dedicated signed-in settings surface** (`/account/settings`) reachable
  from the shared sidebar, hosting the notifications preferences section.
- **FR-2** A **sidebar entry point** (footer user area, alongside `Keyboard shortcuts`)
  that navigates to the settings surface. Present on every authenticated page (the rail
  is shared via `NavContext`).
- **FR-3** The settings surface renders the **existing per-workspace Muted/Subscribed
  list** and the existing **resubscribe** action, reusing the shipped
  `show_notifications` data path (`workspaces_for_member` + `list_unsubscribed_workspace_ids`).
- **FR-4** A signed-in **mute (unsubscribe) action per workspace**, so the surface is a
  complete subscribe/unsubscribe control — not resubscribe-only. (New signed-in POST;
  reuses the shipped `Store::insert_unsubscribe` the public `/unsubscribe` POST already calls.)
- **FR-5** Backwards-compat: the existing `/account/notifications` and public
  `/unsubscribe` routes keep working unchanged (or `/account/notifications` folds into
  the new surface without breaking existing acceptance coverage).

### Out of scope
- Any change to notification **delivery** (providers, transports, fan-out) — that is the
  already-shipped `notification-delivery-providers` feature; untouched here.
- Per-channel routing, digests, quiet-hours, per-event granularity — the mute unit stays
  **per-workspace**, exactly as the shipped backend models it.
- Profile / security / other settings sections — the shell may leave room for them but
  this feature ships only the **Notifications** section.

## Non-Functional Requirements
- **NFR-1 Least privilege / non-enumerable** — identity comes ONLY from `SessionUser`;
  a signed-out caller gets the shipped uniform 404. Any new mute action must match the
  session-scoped, `workspaces_for_member`-checked posture of the shipped resubscribe
  handler (no request-supplied email/workspace can steer another recipient's state).
- **NFR-2 CSRF** — every state-changing form is CSRF-checked by the shipped
  `csrf_middleware`, carrying the per-request `nav.csrf` double-submit token.
- **NFR-3 Exactly-one active primary** — adding the settings link must not break the
  navigation-bar invariant that exactly one primary rail item (`Home`/`Board`) is
  current; settings lives in the footer user area, not as a third primary item.

## Open decisions (for DISTILL/DELIVER to resolve)
- **OD-1** New `/account/settings` route hosting notifications vs. keep
  `/account/notifications` as the canonical URL and just link to it. (User chose a
  *dedicated settings surface*, so lean toward `/account/settings` as the shell with
  notifications as its first section; decide whether `/account/notifications` redirects
  in or is embedded.)
- **OD-2** Sidebar label + placement: `Settings` vs `Notifications` in the footer user block.
- **OD-3** Mute-action UX on the list: per-row `Mute`/`Resubscribe` toggle button.
