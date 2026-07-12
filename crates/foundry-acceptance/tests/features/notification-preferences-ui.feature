# Feature: notification-preferences-ui — make the SHIPPED per-workspace notification
# mute/subscribe backend reachable + self-service from the signed-in UI.
#
# The predecessor feature `recipient-notification-preferences` already ships the
# backend: the signed-in status page (`GET /account/notifications`,
# unsubscribe.rs::show_notifications), signed-in resubscribe
# (`POST /account/notifications/resubscribe`), and the public signed-link
# `/unsubscribe` flow. THIS feature only (1) adds a discoverable sidebar entry point
# to a dedicated settings surface (`GET /account/settings`) hosting the notifications
# section, and (2) adds the missing signed-in per-workspace MUTE action
# (`POST /account/settings/mute`) so the surface is a complete subscribe/unsubscribe
# control — not resubscribe-only. Reuse-over-reinvent: the settings surface renders
# the SHIPPED `workspaces_for_member` + `list_unsubscribed_workspace_ids` data path;
# the mute action mirrors the SHIPPED resubscribe handler's least-privilege,
# session-scoped, CSRF-checked, non-enumerable posture but calls
# `Store::insert_unsubscribe` (idempotent, `ON CONFLICT DO NOTHING`).
#
# Grounding SSOT (seed, not a full DISCUSS/DESIGN wave):
#   docs/feature/notification-preferences-ui/discuss/requirements.md (FR-1..5, NFR-1..3)
#   docs/feature/notification-preferences-ui/design/architecture.md   (the one new shell)
#
# Driving ports (Mandate 1 — every scenario enters through one, never an internal fn):
#   1. The shared sidebar rail on any AUTHENTICATED page (the `<a>` in `sidebar__user`).
#   2. `GET /account/settings` — the signed-in settings surface (uniform 404 signed out).
#   3. `POST /account/settings/mute` — the NEW signed-in per-workspace mute action.
#   4. `POST /account/notifications/resubscribe` — the SHIPPED signed-in resubscribe
#      (regression-guarded: it must keep working, reached from the surface).
#
# HARNESS BOUNDARY: the app + Postgres are REAL (the shipped in-process axum harness
# `support::harness::InProcHarness::spawn` + testcontainers, `@real-io`), driven through
# the production composition root (`build_router`) — Pillar 3. The `/account/settings`
# GET, the new mute POST, and the sidebar link are exercised REAL through reqwest. No
# email delivery is observed here (that is the shipped recipient-unsubscribe feature),
# so no recording provider double is needed — the observable is the rendered surface +
# the `notification_unsubscribes` opt-out state read at the store boundary.
#
# EVERY scenario is @pending, PER-SCENARIO (never a feature-level tag): @pending is
# excluded from EVERY lane (acceptance.rs filter_run, `!has("pending")`), so this file
# keeps the default + @all lanes green until DELIVER unskips slice-by-slice and turns
# each GREEN (Outside-In). Run this feature's lane with
# `FOUNDRY_ACCEPTANCE_TAGS=notification-preferences-ui`. Reuses the shipped
# navigation-bar-linear-ui step glue (`(\w+) opens the authenticated page "…"`,
# `exactly one primary navigation item is marked as the current page`, `the "…"
# navigation item is marked as the current page`) so the exactly-one-active-primary
# invariant is asserted with the SAME oracle the nav sweep already uses (NFR-3).
#
# OPEN DECISIONS carried to DELIVER (from the seed docs):
#   OD-1 `/account/settings` as the canonical shell vs. folding in `/account/notifications`.
#        These scenarios pin the OBSERVABLE surface at `/account/settings`; whether the
#        old URL redirects in or embeds is a DELIVER wiring choice below the assertions.
#   OD-2 sidebar label ("Settings" vs "Notifications") — the link is asserted by its
#        `href="/account/settings"`, NOT its text, so DELIVER may choose either label.
#   OD-3 the mute POST path — pinned to `/account/settings/mute` here; if DELIVER lands a
#        different path, this file + `feature_notification_preferences_ui.rs` move together.

@notification-preferences-ui @driving_port
Feature: A signed-in member manages their notification preferences from inside the app
  A signed-in member can find and change which workspaces email them from a discoverable
  settings surface reachable in the sidebar — muting a noisy workspace or resuming a
  muted one — without hunting for an old message's unsubscribe link, safely
  (least-privilege, session-scoped, non-enumerable) and without disturbing the app's
  single-active-primary navigation.

  # ── Slice 01 — US-01 walking skeleton: sidebar link → settings surface → mute ──

  @us-01 @walking_skeleton @driving_port @real-io
  Scenario: A member reaches notification settings from the sidebar and mutes a workspace
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia opens an authenticated page and follows the settings link in the sidebar
    Then Nadia sees the notification settings surface listing "Northwind", "Contoso", and "Initech"
    When Nadia mutes "Northwind" from the settings surface
    Then "Northwind" is shown as muted on the settings surface
    And "Contoso" is shown as subscribed on the settings surface

  # ── Slice 02 — US-01 the discoverable entry point (sidebar footer) ────────────

  @us-01 @driving_port @real-io
  Scenario: The sidebar footer offers a link to notification settings
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia opens an authenticated page
    Then the sidebar footer offers a link to the notification settings surface

  @us-01 @property @driving_port @real-io
  Scenario: Reaching settings keeps Home the only current primary navigation item
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia opens the authenticated page "/"
    Then the "Home" navigation item is marked as the current page
    And exactly one primary navigation item is marked as the current page
    And the sidebar footer offers a link to the notification settings surface

  # ── Slice 03 — US-02 the settings surface renders the shipped status list ─────

  @us-02 @driving_port @real-io
  Scenario: The settings surface shows each workspace's mute status
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    And Nadia has muted "Northwind"
    When Nadia opens the notification settings surface
    Then Nadia sees the notification settings surface listing "Northwind", "Contoso", and "Initech"
    And "Northwind" is shown as muted on the settings surface
    And "Contoso" is shown as subscribed on the settings surface
    And "Initech" is shown as subscribed on the settings surface

  @us-02 @error @security @driving_port @real-io
  Scenario: A signed-out visitor cannot see the settings surface
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When a signed-out visitor opens the notification settings surface
    Then the notification settings surface is not shown

  # ── Slice 04 — US-03 the new signed-in per-workspace mute action ──────────────

  @us-03 @driving_port @real-io
  Scenario: A subscribed workspace can be muted from the settings surface
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia mutes "Contoso" from the settings surface
    Then "Contoso" is shown as muted on the settings surface
    And "Northwind" is shown as subscribed on the settings surface

  @us-03 @error @security @real-io
  Scenario: A mute without a valid request token is refused
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia tries to mute "Northwind" without a valid request token
    Then the mute is refused and Nadia's notification state is unchanged

  @us-03 @error @real-io
  Scenario: Muting an already-muted workspace twice is harmless
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia mutes "Northwind" twice from a stale surface
    Then Nadia sees the same mute confirmation both times with no error
    And "Northwind" is shown as muted on the settings surface

  @us-03 @error @security @real-io
  Scenario: Muting a workspace the member does not belong to is refused non-enumerably
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Nadia tries to mute a workspace she does not belong to
    Then the mute is refused without revealing whether the workspace exists

  @us-03 @security @real-io
  Scenario: A crafted foreign workspace cannot steer another recipient's state
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When a crafted request tries to mute a workspace belonging to another recipient
    Then the mute is refused without revealing whether the workspace exists
    And the other recipient's notification state is unchanged

  @us-03 @error @security @real-io
  Scenario: A signed-out visitor cannot mute a workspace
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When a signed-out visitor tries to mute a workspace
    Then the mute is refused and no notification state changes

  # ── Slice 05 — US-04 the shipped resubscribe still works from the surface ─────

  @us-04 @driving_port @real-io
  Scenario: A muted workspace can be resubscribed from the settings surface
    Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"
    And Nadia has muted "Northwind"
    When Nadia resubscribes to "Northwind" from the settings surface
    Then "Northwind" is shown as subscribed on the settings surface
