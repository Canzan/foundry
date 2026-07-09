# Feature: navigation-bar-linear-ui — one shared Linear-style left sidebar on
# authenticated app pages; pre-auth/util pages stay chrome-free.
#
# Source SSOT for docs/feature/navigation-bar-linear-ui/distill/test-scenarios.md.
# A shared `app_shell.html` layout injects a `partials/sidebar.html` rail on every
# AUTHENTICATED full-page surface (dashboard, board, report, tokens, invites,
# instance admin, …). The rail carries workspace identity + monogram (top), the two
# primary items Home (`/`) and Board (deep-link to the first project board, active on
# the `/team/*/project/*` family), and a footer user menu (identity anchor, Keyboard
# shortcuts, Sign out, and an Instance-admin item gated on `is_instance_admin`).
# Pre-auth pages keep `{% extends "base.html" %}` — chrome-free is STRUCTURAL, not a
# flag (DESIGN ADR-001).
#
# Driving port: HTTP GET on the authed routes (`/`, `/team/{s}/project/{s}`,
# `.../report`, `/admin/tokens`, `/workspace/invites`) + the reused CSRF-protected
# `POST /sign-out`; pre-auth GET on `/sign-in` + `/forgot-password`.
# Driven adapters: real Postgres (users, workspaces, teams, projects,
# workspace_memberships, instance_admins, tower_sessions) via testcontainers.
#
# EVERY scenario is @pending: DELIVER removes the tag per-scenario as it authors the
# nav production code (src/nav.rs, app_shell.html, partials/sidebar.html) and turns
# it GREEN (Outside-In). @pending is excluded from every lane (acceptance.rs
# filter_run), so this file keeps the @all lane green until then.
#
# Personas + Background reuse the dashboard-enhancements phrasings VERBATIM
# (`a workspace "Acme" … admin "Ada" … "Ada Lovelace"`, `a project … exists in …`,
# `Ada is signed in`) so DELIVER shares one set of Given/When glue across both
# features. Board slugs follow the Background seed: team "general", project "sandbox".

@navigation-bar @us-nav @driving_port
Feature: A signed-in member navigates every authenticated page from one shared sidebar
  The shared rail gives the member a single, consistent place to see where they are
  (active section), jump between Home and their project board in one click, and reach
  account actions (keyboard help, sign out, instance admin) — a Linear-feel chrome
  that is present on every authed page and absent on pre-auth pages.

  Background:
    Given a workspace "Acme" exists with admin "Ada" and display name "Ada Lovelace"
    And a project "Sandbox" with key prefix "GEN" exists in "Acme"
    And Ada is signed in

  # ── Slice 01 — US-01 walking skeleton: rail on the dashboard, Home current ──
  @us-01 @walking_skeleton @real-io
  Scenario: The dashboard shows the shared sidebar with Home current
    When Ada visits "/"
    Then a persistent left sidebar is shown
    And the sidebar shows the workspace name "Acme"
    And the sidebar shows primary navigation items "Home" and "Board"
    And the "Home" navigation item is marked as the current page
    And the "Board" navigation item is not marked as current

  # ── Slice 02 — US-04 presence across the authenticated page set ─────────────
  @us-04 @real-io
  Scenario Outline: The shared sidebar is present on every authenticated app page
    When Ada opens the authenticated page "<page>"
    Then a persistent left sidebar is shown
    And the sidebar shows primary navigation items "Home" and "Board"

    Examples:
      | page                                    |
      | /                                       |
      | /team/general/project/sandbox           |
      | /team/general/project/sandbox/report    |
      | /admin/tokens                           |
      | /workspace/invites                      |

  # ── Slice 03 — US-01/US-06 active-state correctness (the design's #1 risk) ──
  @us-01 @real-io
  Scenario: Board is current while viewing a project board
    When Ada opens the authenticated page "/team/general/project/sandbox"
    Then the "Board" navigation item is marked as the current page
    And the "Home" navigation item is not marked as current

  @property @us-01 @us-06 @real-io
  Scenario Outline: Exactly one primary item is current on every authenticated page
    When Ada opens the authenticated page "<page>"
    Then exactly one primary navigation item is marked as the current page

    Examples:
      | page                                    |
      | /                                       |
      | /team/general/project/sandbox           |
      | /team/general/project/sandbox/report    |
      | /admin/tokens                           |
      | /workspace/invites                      |

  @us-06 @real-io
  Scenario: The active item is an accessible landmark carrying aria-current
    When Ada opens the authenticated page "/team/general/project/sandbox"
    Then the sidebar is exposed as a navigation landmark
    And the current navigation item carries an aria-current marker

  # ── Slice 04 — US-01 absence on pre-auth / utility pages (structural) ───────
  @us-01 @real-io
  Scenario Outline: Pre-auth and utility pages do not show the sidebar
    Given a visitor is not signed in
    When a visitor opens the pre-auth page "<page>"
    Then no navigation sidebar is shown
    And only the page's own content is visible

    Examples:
      | page             |
      | /sign-in         |
      | /forgot-password |

  # ── Slice 05 — US-02 account actions in the footer user menu ────────────────
  @us-02 @real-io
  Scenario: The user menu links to keyboard shortcuts
    When Ada visits "/"
    Then the user menu contains a link to "/keyboard-help"

  @us-02 @real-io @security
  Scenario: The user menu signs out with a CSRF token
    When Ada visits "/"
    Then the user menu contains a sign-out control posting to "/sign-out" with a CSRF token

  # ── Slice 06 — US-03 instance-admin gating (mirror dashboard's two-way gate) ─
  @pending @us-03 @real-io
  Scenario: A super-admin sees the Instance admin item in the user menu
    Given Ada is an instance super-admin
    When Ada visits "/"
    Then the user menu contains a link to "/admin/instance/workspaces"

  @pending @us-03 @real-io @security
  Scenario: A non-super-admin never sees the Instance admin item
    Given a member "Mei" who is not an instance admin is signed in
    When Mei visits "/"
    Then the user menu does not contain a link to "/admin/instance/workspaces"

  # ── Slice 07 — US-04 rail identity + inert markup ───────────────────────────
  @pending @us-04 @real-io
  Scenario: The rail renders the workspace name and signed-in identity
    When Ada visits "/"
    Then the sidebar shows the workspace name "Acme"
    And the sidebar footer shows the signed-in name "Ada Lovelace"

  @pending @us-04 @real-io @security
  Scenario: A display name containing markup is rendered inert in the rail
    Given a member "Mallory" whose display name is "<b>pwn</b>" is signed in
    When Mallory visits "/"
    Then the response body contains the escaped display name
    And the response body does not contain a live "<b>" element

  # ── Slice 08 — US-01 Board deep-link target (ADR-003) ───────────────────────
  @pending @us-01 @real-io
  Scenario: The Board item deep-links to the workspace's first project board
    When Ada visits "/"
    Then the sidebar links "Board" to "/team/general/project/sandbox"

  @pending @us-01 @real-io
  Scenario: The Board item falls back to the dashboard when there are no projects
    Given the "Acme" workspace has no projects
    When Ada visits "/"
    Then the sidebar links "Board" to "/"

  # ── Slice 09 — US-05 scoping guard: Quick actions preserved, not promoted ───
  @us-05 @real-io
  Scenario: Invites and machine tokens stay in Quick actions and are not promoted
    When Ada visits "/"
    Then the response body contains a link to "/workspace/invites"
    And the response body contains a link to "/admin/tokens"
    And the sidebar does not contain a "Invite a member" item
    And the sidebar does not contain a "Machine tokens" item
