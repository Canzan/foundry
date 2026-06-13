# Feature: web-provisioning-flow — the WEB provisioning surface (the web legs of
# US-MWT07 + US-MWT08). An instance super-admin, signed in to the browser, gains a
# super-admin-gated /admin/instance/… surface to provision a new isolated workspace
# and to grant super-admin — a NEW WEB DRIVING ADAPTER over the ALREADY-SHIPPED
# `provision_workspace` use-case + `is_instance_admin` authz. The CLI legs shipped in
# us-mwt-slice-06-provision-and-prove.feature; this feature adds the browser surface
# the CLI-first v1 deferred (multi-workspace-provisioning ADR-002 D2 → realised HERE;
# multi-workspace-tenancy ADR-004 option (d)).
#
# Hypothesis (design/architecture.md G1-G9, ADR-001..005): a non-shell super-admin can
# provision a workspace + grant super-admin from the browser, gated by the SHIPPED
# `is_instance_admin`, reusing the SHIPPED session + double-submit-CSRF machinery and the
# SHIPPED non-enumerable uniform-404 refusal idiom — and we KNOW we are right when the
# provision form creates a real isolated tenant via the shipped use-case (leaving existing
# workspaces untouched), the grant is idempotent, and BOTH a signed-out request AND a
# signed-in non-super-admin request to EVERY /admin/instance/… route (GET and POST) are
# refused with a uniform 404 that is BYTE-IDENTICAL to a never-existed path (no 403, no
# login redirect, no oracle that the surface exists). DISPROVED if the web surface creates
# a workspace for a non-super-admin, leaks the surface's existence to an unauthorised
# caller, accepts a CSRF-less provision POST, leaves a second live "create workspace" POST,
# or touches an existing tenant when provisioning a new one.
#
# Driving adapter: the htmx web tier served by foundry-app over real HTTP (the in-process
# axum router / InProcHarness the Feature-B + slice-02/04 web scenarios drive), under the
# production session + double-submit CSRF layers (a browser human), reached at the THREE
# new routes (ADR-001 / D1):
#   GET  /admin/instance/workspaces      (instance dashboard: workspace list + provision form + grant form)
#   POST /admin/instance/workspaces      (provision: name + first-admin email + _csrf → htmx success fragment)
#   POST /admin/instance/super-admins    (grant super-admin: email + _csrf → htmx confirmation fragment)
# authenticated by a real signed-in `foundry_session` cookie whose user is (or is not) an
# `instance_admins` row. The provisioning leg converges on the SHIPPED
# `Services::provision_workspace`; the grant leg on the SHIPPED
# `grant_instance_admin` + `user_id_by_email`. The "first admin can act" leg, where
# asserted, rides the SHIPPED `resolve_active_workspace` membership seam (as slice-06 did),
# NOT a real invite-accept sign-in (D5 — see RED-state contract).
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces, users,
# workspace_memberships, invites, instance_admins) via testcontainers + per-scenario schema;
# the real tower-sessions Postgres store (signed-in `foundry_session`); the real
# double-submit CSRF middleware (`csrf_middleware`); the SHIPPED `Services::provision_workspace`
# use-case (incl. its own fail-closed `is_instance_admin` re-check, defence-in-depth) + its
# atomic create+seed tx; the SHIPPED `is_instance_admin` authz; the SHIPPED
# `grant_instance_admin` (idempotent) + `user_id_by_email`; the thin new non-tenant-scoped
# `list_workspaces` read (D4); the in-process axum router. NO mocks at the acceptance level.
#
# Non-enumerability decision (ADR-002 response-mapping contract table — the security CORE):
# `require_instance_admin` resolves to EXACTLY ONE of two outcomes, and BOTH refusal cases
# return the SAME `resource_not_found_page()` — IDENTICAL status 404 + IDENTICAL body:
#   * No SessionUser (signed-out)                       ⇒ uniform 404
#   * SessionUser present, is_instance_admin == false   ⇒ uniform 404 (BYTE-IDENTICAL to signed-out)
#   * SessionUser present, is_instance_admin == true    ⇒ pass (handler runs)
# There is NO third response shape, NO 403, NO 401, and NO redirect-to-login that varies by
# WHICH refusal occurred — that uniformity IS the non-enumerability property. The scenarios
# assert it BYTE-IDENTICALLY (status + body), against a never-existed path as the control, on
# EVERY route (GET + both POSTs) for BOTH refusal causes — and the slice-04 lesson is honoured
# (slice-04 found 4 oracles): identity of refusal is asserted on the full response (status AND
# body bytes), never merely same-status. The grant form is likewise NOT a user-enumeration
# oracle: an unknown email yields the SAME non-committal confirmation as a known one (D2 (g)).
# DELIVER asserts via a revert-reds-it litmus (collapsing the two refusal arms into distinct
# responses — a 401/403 for one — MUST re-RED the byte-identity assertion).
#
# Legacy-route-retired decision (ADR-003 / D3, RATIFIED RETIRE): the legacy identity-blind
# `POST /workspaces` 409 guard (`bootstrap.rs:301`) is DELETED outright (not left inert), per
# the repo AGENTS.md "## Dead code" policy (pre-stable: remove superseded code). The gated
# `POST /admin/instance/workspaces` is the SOLE web provisioning path; the bootstrap CLAIM
# remains the sole creator of workspace 1. Scenario 9 asserts the legacy route is GONE
# (a POST to /workspaces is now refused as a never-existed path, not a 409).
#
# Invite-accept scope (ADR-005 / D5, RATIFIED OUT of v1): there is NO `/invites/accept`
# route, no `consume_invite` store fn, no password-set handler — the emitted invite link is a
# DEAD URL today (for the CLI as much as the web). NO scenario here requires the provisioned
# first-admin to actually sign in via the link. The success fragment asserts the link is
# RENDERED (informational, marked pending); the "first admin can act" property, where
# asserted at all, uses the SHIPPED `resolve_active_workspace` membership seam (the same
# approximation slice-06 used), NOT a live accept-and-set-password flow.
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (feature files are
# Gherkin text; this file adds NO undefined-symbol reference to any .rs and does NOT edit
# acceptance.rs) → NOT BROKEN. Genuine RED is MISSING_FUNCTIONALITY at runtime against the
# real testcontainers PG16:
#   1. The `instance_admin.rs` web adapter, the `require_instance_admin` gate, the three
#      `/admin/instance/…` routes, the 2-3 Askama templates, and the thin `list_workspaces`
#      read DO NOT EXIST YET — every web scenario fails because the route is unknown / the
#      handler is absent. That is the genuine RED for the NEW surface.
#   2. The non-enumerability scenarios red because there is no gate to refuse uniformly yet;
#      once the gate lands they pass green-by-inheritance off the SHIPPED `resource_not_found_page()`
#      idiom (the shipped `/workspace/switch` + `/admin/tokens` precedent).
#   3. The "leaves existing workspaces untouched" + "first admin can act" legs ride the
#      SHIPPED provisioning use-case + isolation boundary (slices 1-6, green) — once the web
#      adapter can drive provisioning at all, they assert the SHIPPED behaviour holds for the
#      web-provisioned tenant (green-by-inheritance behind the new adapter), they do not
#      require new domain/isolation code.
#   4. Scenario 9 (legacy route retired) reds while `POST /workspaces` still 409s; it greens
#      when DELIVER DELETES the route (D3) and the path returns the never-existed refusal.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter scenarios are
# example-based (NOT property-based); every sad / evil-user / unauthorised path is enumerated
# explicitly; no PBT machinery at this layer. Mandate 8 state-delta is layers 1-3 with a
# Python pilot port; no `state_delta.rs` Rust port exists (matching slices 1-6), so LAYER-3
# assertions are traditional assertions over port-exposed web observables: rendered page /
# fragment substrings (new workspace id, invite link, confirmation), HTTP refusal status +
# BYTE-IDENTICAL refusal body, CSRF rejection status, and post-provision DB row presence
# scoped by workspace (+ the unchanged existing-workspace snapshot).
#
# Scope: the WEB provisioning + grant surface ONLY (the web legs of US-MWT07/08). The CLI
# provisioning surface (us-mwt-slice-06), the shipped isolation core (slices 1-4), the
# migration guarantee (slice 5), the rate-bucket eviction (slice 6b, a unit/property test),
# and the deferred invite-accept vertical (D5, OUT) are explicitly OUT — do not add them here.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time DELIVER
# cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@web-provisioning-flow @real-io @driving_adapter
Feature: An instance super-admin provisions a new workspace and grants super-admin from the browser
  Sasha claimed her Foundry instance at bootstrap, so she is both workspace 1's
  admin and the first instance super-admin. She is not a shell user, so the
  CLI-first v1 left her no path to provision. From the browser she opens the
  instance dashboard, fills the provision form, and a new isolated workspace
  "Globex" is created via the shipped use-case — its id and first-admin invite
  link shown back to her; she can also grant another operator super-admin from the
  same page. Creating Globex leaves Acme untouched. Anyone who is not a super-admin
  — signed out OR signed in as an ordinary member — cannot even tell the surface
  exists: every /admin/instance/… request is refused with a uniform 404 byte-for-byte
  identical to a path that never existed. Provision POSTs without a valid CSRF token
  are refused. The legacy identity-blind /workspaces 409 route is gone. Proven with
  REAL fixtures over the real session + CSRF + Postgres machinery, not synthetic ids.

  Background:
    Given an instance claimed by super-admin "ops@acme.com" with workspace "Acme"
    And "Acme" has a member "marco@acme.com"

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able headline value, end-to-end through the web
  #    driving adapter. "A signed-in super-admin submits the web provision form and
  #    a new isolated workspace is created; the page reports the new workspace and
  #    its first-admin invite link." The thinnest cut that proves the new web route
  #    wires through the session + CSRF layers to the SHIPPED provisioning use-case
  #    and back to a rendered htmx success fragment. (US-MWT07 web leg.)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-mwt07
  Scenario: A super-admin provisions a new isolated workspace from the browser
    Given the super-admin is signed in on the web
    When the super-admin submits the provision form for workspace "Globex" with first admin "priya@globex.com"
    Then the new workspace "Globex" exists and is isolated from all others
    And the web page reports the new workspace and a first-admin invite link

  # ----------------------------------------------------------------------------
  # 2. The instance dashboard renders for a super-admin (ADR-001 / D1): the GET
  #    page shows the existing workspace list plus the provision form and the grant
  #    form. The full-page (no-JS) entry point of the surface.
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: The instance dashboard shows the workspace list and the provision and grant forms
    Given the super-admin is signed in on the web
    When the super-admin opens the instance dashboard on the web
    Then the dashboard lists the existing workspaces
    And the dashboard offers a provision-workspace form and a grant-super-admin form

  # ----------------------------------------------------------------------------
  # 3. Grant super-admin from the browser (ADR-001 / D1, ADR-004 reuse) — the grant
  #    form drives the SHIPPED idempotent grant. The granted operator becomes a
  #    super-admin. (US-MWT07 web leg — the grant operation.)
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: A super-admin grants super-admin to another operator from the browser
    Given the super-admin is signed in on the web
    And "dana@acme.com" is an existing member who is not a super-admin
    When the super-admin submits the grant form for "dana@acme.com"
    Then the web page confirms the grant
    And "dana@acme.com" is now a super-admin

  # ----------------------------------------------------------------------------
  # 4. The grant is idempotent (ADR-004 / G2 — INSERT … ON CONFLICT DO NOTHING).
  #    Granting the same operator twice from the browser records the role exactly
  #    once and is confirmed both times.
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: Granting super-admin twice from the browser is idempotent
    Given the super-admin is signed in on the web
    And "dana@acme.com" is an existing member who is not a super-admin
    When the super-admin submits the grant form for "dana@acme.com"
    And the super-admin submits the grant form for "dana@acme.com" a second time
    Then the web page confirms the grant both times
    And "dana@acme.com" is recorded as a super-admin exactly once

  # ----------------------------------------------------------------------------
  # 5. The grant form is not a user-enumeration oracle (ADR-002 (g) / D2). A grant
  #    for an unknown email returns the SAME non-committal confirmation as a known
  #    one — the response carries no oracle for whether the email belongs to a real
  #    user. (Evil-user / non-enumerability of the grant surface.)
  # ----------------------------------------------------------------------------
  @us-mwt07 @error
  Scenario: Granting an unknown email does not reveal whether the user exists
    Given the super-admin is signed in on the web
    And "dana@acme.com" is an existing member who is not a super-admin
    When the super-admin submits the grant form for the existing email "dana@acme.com"
    And the super-admin submits the grant form for an email that belongs to no user
    Then the two grant responses are confirmed identically
    And neither response reveals whether the email belongs to a real user

  # ----------------------------------------------------------------------------
  # 6. Non-enumerable refusal — SIGNED-OUT (ADR-002 response-mapping, the security
  #    core). A signed-out request to EVERY /admin/instance/… route (the GET page,
  #    the provision POST, the grant POST) is refused with a uniform 404 that is
  #    BYTE-IDENTICAL to a never-existed path — no 403, no 401, no login redirect.
  #    The control is a path that never existed. (Evil-user.)
  # ----------------------------------------------------------------------------
  @us-mwt08 @error
  Scenario: A signed-out request to the admin surface is refused like a path that never existed
    Given no user is signed in on the web
    When a signed-out caller requests each /admin/instance route on the web
    And a signed-out caller requests a path that never existed on the web
    Then every admin-surface response is refused identically to the never-existed path
    And nothing reveals that the admin surface exists

  # ----------------------------------------------------------------------------
  # 7. Non-enumerable refusal — SIGNED-IN NON-SUPER-ADMIN (ADR-002 response-mapping,
  #    the security core). An ordinary signed-in member's request to EVERY
  #    /admin/instance/… route is refused with a 404 that is BYTE-IDENTICAL to the
  #    signed-out refusal AND to a never-existed path — the non-super-admin learns
  #    nothing about whether the surface (or target) exists. (Evil-user.)
  # ----------------------------------------------------------------------------
  @pending @us-mwt08 @error
  Scenario: A signed-in non-super-admin request to the admin surface is refused non-enumerably
    Given "marco@acme.com" is signed in on the web and is not a super-admin
    When the member requests each /admin/instance route on the web
    And the member requests a path that never existed on the web
    Then every admin-surface response is refused identically to the never-existed path
    And the non-super-admin refusal is byte-identical to the signed-out refusal
    And nothing reveals that the admin surface exists

  # ----------------------------------------------------------------------------
  # 8. CSRF — a provision POST without a valid double-submit token is refused (G5 /
  #    ADR-002). The shipped csrf_middleware refuses the state-changing POST, and no
  #    workspace is created. (Evil-user / the shipped CSRF guard exercised on the
  #    new route.)
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @error
  Scenario: A provision request without a valid security token is refused
    Given the super-admin is signed in on the web
    When the super-admin submits the provision form for workspace "Globex" without a valid security token
    Then the provision request is refused
    And no new workspace was created

  # ----------------------------------------------------------------------------
  # 9. The legacy identity-blind /workspaces 409 route is RETIRED (ADR-003 / D3,
  #    RATIFIED RETIRE — DELETED, not inert). A POST to the old /workspaces path is
  #    now refused as a path that never existed (no 409), proving the gated
  #    /admin/instance/workspaces POST is the SOLE web provisioning path.
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @error @verify-path-unchanged
  Scenario: The legacy create-workspace route no longer exists
    Given the super-admin is signed in on the web
    When the super-admin posts to the legacy create-workspace path on the web
    Then the legacy path is refused like a path that never existed
    And the legacy path does not answer with the old conflict response

  # ----------------------------------------------------------------------------
  # 10. Provisioning via the web leaves existing workspaces untouched
  #     (NFR-MWT-REL-01, green-by-inheritance from slice-06). Snapshot Acme before,
  #     provision Globex from the browser, assert Acme is byte-for-byte unchanged
  #     and Globex starts empty and isolated. (US-MWT07 / US-MWT08 web legs.)
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @us-mwt08
  Scenario: Provisioning from the browser leaves existing workspaces untouched
    Given the super-admin is signed in on the web
    And a recorded snapshot of "Acme" and its data and members
    When the super-admin submits the provision form for workspace "Globex" with first admin "priya@globex.com"
    Then "Acme" and all its data and members are unchanged
    And "Globex" starts empty and isolated

  # ----------------------------------------------------------------------------
  # 11. The web-provisioned tenant honours the SHIPPED isolation boundary
  #     (NFR-MWT-SEC-01, green-by-inheritance). The first admin of the
  #     browser-provisioned Globex acts only on Globex; no Acme data appears. The
  #     "first admin can act" leg rides the SHIPPED resolve_active_workspace
  #     membership seam (D5 — NOT a real invite-accept sign-in). (US-MWT08 web leg.)
  # ----------------------------------------------------------------------------
  @pending @us-mwt08
  Scenario: The browser-provisioned workspace is a real isolated tenant
    Given the super-admin has provisioned workspace "Globex" from the browser with first admin "priya@globex.com"
    And "Globex" has issues that belong to "Globex"
    When the first admin of "Globex" lists her issues through the membership seam
    Then she sees only "Globex" issues
    And no "Acme" issue appears
