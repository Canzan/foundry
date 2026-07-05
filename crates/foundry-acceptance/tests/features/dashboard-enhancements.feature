# Feature: dashboard-enhancements — the signed-in landing ("/") is rounded out.
#
# Source SSOT for docs/feature/dashboard-enhancements/distill/test-scenarios.md.
# The base dashboard (project list + quick actions) shipped in 51ba981; this
# feature adds a personalized greeting, a super-admin-only instance link, a
# sign-out control, and backfills coverage.
#
# Driving port: HTTP GET/POST on "/" and "/sign-out" (session-cookie auth).
# Driven adapters: real Postgres (users, workspaces, teams, projects,
# instance_admins, tower_sessions) via testcontainers.
#
# EVERY scenario is @pending: DELIVER removes the tag per-scenario as it authors
# the step glue and turns it GREEN (Outside-In). @pending is excluded from every
# lane (acceptance.rs filter_run), so this file keeps the @all lane green until
# then.

@dashboard-enhancements @us-dash @driving_port
Feature: A signed-in user orients, navigates by role, and signs out from the dashboard
  The signed-in landing greets the user by name, names their workspace, lists
  their projects, exposes only the tools their role grants, and lets them sign
  out — a Linear-feel home base rather than a bare "you are signed in" stub.

  Background:
    Given a workspace "Acme" exists with admin "Ada" and display name "Ada Lovelace"
    And a project "Sandbox" with key prefix "GEN" exists in "Acme"
    And Ada is signed in

  # ── Slice 01 — US-01 greeting ─────────────────────────────────────────────
  @pending @us-01 @walking_skeleton @real-io
  Scenario: The dashboard greets the user by name and names the workspace
    When Ada visits "/"
    Then the response body contains "Ada Lovelace"
    And the response body contains "Acme"
    And the response body contains the heading "Foundry"

  @pending @us-01 @real-io @security
  Scenario: A display name containing markup is rendered inert
    Given a member "Mallory" whose display name is "<b>pwn</b>" is signed in
    When Mallory visits "/"
    Then the response body contains the escaped display name
    And the response body does not contain a live "<b>" element

  @pending @us-01 @error @real-io
  Scenario: The greeting degrades to 200 if identity cannot be loaded
    Given the identity lookup for the signed-in user fails
    When Ada visits "/"
    Then the response status is 200
    And the response body contains a neutral greeting

  # ── Slice 02 — US-03 instance-admin link (super-admin only) ───────────────
  @pending @us-03 @real-io
  Scenario: A super-admin sees the instance-admin link
    Given Ada is an instance super-admin
    When Ada visits "/"
    Then the response body contains a link to "/admin/instance/workspaces"

  @pending @us-03 @real-io @security
  Scenario: A non-super-admin never sees the instance-admin link
    Given a member "Mei" who is not an instance admin is signed in
    When Mei visits "/"
    Then the response body does not contain a link to "/admin/instance/workspaces"

  # ── Slice 03 — US-02 sign out ─────────────────────────────────────────────
  @pending @us-02 @real-io
  Scenario: A signed-in user signs out from the dashboard
    When Ada visits "/"
    Then the response body contains a sign-out form posting to "/sign-out"
    And the sign-out form carries a "_csrf" token matching the "foundry_csrf" cookie
    When Ada submits the sign-out form
    Then the response redirects Ada to "/sign-in"
    And requesting "/" with the old session redirects to "/sign-in"

  @pending @us-02 @error @real-io
  Scenario: Sign-out with a forged CSRF token is refused
    When Ada posts to "/sign-out" with a "_csrf" that does not match the cookie
    Then the request is refused by CSRF middleware
    And Ada's session is still valid

  # ── Slice 04 — US-05 coverage + US-04 style promotion ─────────────────────
  @pending @us-05 @real-io
  Scenario: The signed-in dashboard lists projects and links to a board
    When Ada visits "/"
    Then the response body contains a project card "GEN" for "Sandbox"
    And that card links to "/team/general/project/sandbox"

  @pending @us-04 @real-io @refactor
  Scenario: Dashboard styles are served from the vendored stylesheet, not inline
    When Ada visits "/"
    Then the response body contains no inline "<style>" block
    And the base layout links a hashed "/static/css/foundry.*.css" stylesheet
    And fetching that stylesheet returns 200 and contains the ".dash" rules
