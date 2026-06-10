# Feature: multi-workspace-tenancy — Slice 2: tenant-scoped authz + non-enumerable
# refusal on the WEB htmx tier. Slice 1 proved coexistence + resolution on the JSON
# API leg; this slice generalises the SAME isolation boundary to the browser/htmx
# web tier — the surface with the MOST read/write paths, so a scoping gap is most
# likely to surface here (slice-02-web-tier-boundary.md "riskiest assumption").
#
# Hypothesis (slices/slice-02-web-tier-boundary.md): the shipped `workspace_id`
# scoping + `is_workspace_admin` + the attachments-style non-enumerable lookup,
# driven by REAL Acme/Globex fixtures, refuse a member of Acme who reaches for a
# Globex resource IDENTICALLY to a non-existent one across the web tier — and an
# Acme admin cannot manage Globex. Disproved if ANY web read/write path leaks
# Globex to Acme.
#
# Driving adapter: the htmx web tier served by foundry-app over real HTTP, under
# the production session + double-submit CSRF layers (a browser human), reached at:
#   GET  /team/{team}/project/{project}                  (board read)
#   GET  /team/{team}/project/{project}/issues/{n}        (issue detail read)
#   POST /team/{team}/project/{project}/issues            (file-issue write)
#   POST /admin/tokens/{jti}/revoke                       (admin action, gated)
# authenticated by a real signed-in `foundry_session` cookie whose acting
# workspace is the session-resolved active workspace (ADR-001 web leg / ADR-005).
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces, users,
# workspace_memberships, teams, team_memberships, projects, issues, machine_tokens)
# via testcontainers + per-scenario schema; the real tower-sessions Postgres store;
# real double-submit CSRF; the in-process axum router (the SAME InProcHarness the
# Feature-B + machine-token-admin scenarios use). The `0002_multi_workspace.sql`
# migration runs as part of the per-scenario schema migration set.
#
# Refusal-status decision (ADR-003 / OD-MWT-D6, confirmed): on the web, a request
# for a resource OUTSIDE the acting workspace returns the SAME 404 not-found
# response (status + page shape) as a never-existed id — generalising the shipped
# `find_*_in_workspace → None` idiom. Cross-tenant resource access NEVER 403s
# (a 403-vs-404 difference would be an existence oracle). The shipped
# `/admin/tokens` surface already collapses a non-admin / missing / foreign jti to
# the SAME non-enumerable 404 (admin_tokens.rs:48, NFR-MT-SEC-03) — slice 2 proves
# it holds when "foreign" means a genuinely-coexisting second workspace.
#   (Intra-workspace authz failures — e.g. a member who is not a team member of
#    their OWN workspace's team — retain their shipped shape; this slice governs
#    CROSS-tenant refusals only, per ADR-003's boundary clause.)
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (no
# import/collection error → not BROKEN). At runtime against the real testcontainers
# PG16, the genuine RED is MISSING_FUNCTIONALITY:
#   1. The Background seeds Acme then Globex; the SECOND `INSERT INTO workspaces`
#      FAILS on `uniq_one_workspace` (0001_init.sql:15) until DELIVER ships `0002`.
#   2. Once two workspaces coexist, the web session's acting workspace is resolved
#      by `first_workspace()` (signin.rs:140) — which picks an ARBITRARY workspace
#      under two rows, so a member of Acme is NOT reliably scoped to Acme. ADR-005
#      replaces that call-site with membership resolution + the switcher; until
#      DELIVER wires it, the isolation + switch scenarios red for the right reason.
#   3. The `ActingWorkspace` newtype + the NEW check-arch LAYER-1e tenant-scoping
#      rule (ADR-002) land WITH this slice; until then the structural guard is
#      absent. (The check-arch gold test is Slice 3+ scope per ADR-002; this slice
#      proves the BEHAVIOURAL boundary.)
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter
# scenarios → example-based (NOT property-based); every sad/evil-user path is
# enumerated explicitly; no PBT machinery at this layer. Mandate 8 state-delta is
# layers 1-3; at layer 3 traditional assertions over port-exposed web observables
# (rendered page substrings, HTTP refusal status, post-write DB row presence
# scoped by workspace) are used per the Layered Test Discipline table.
#
# Scope: SLICE 2 ONLY — the WEB htmx tier. The JSON /api/v1 + machine-token +
# sign-in resolution surfaces (Slice 3), the uniform non-enumerability matrix
# across ALL surfaces + full adversarial coverage (Slice 4), migration-as-
# guarantee (Slice 5), and provisioning (Slice 6) are explicitly OUT — do not add
# them here.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time
# DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@multi-workspace-tenancy @mwt-slice-02 @real-io @driving_adapter @us-mwt02
Feature: A member or admin of one workspace cannot read, write, or manage another's data on the web
  Marco is a member of "Acme"; Priya is an admin of "Acme" but not of "Globex".
  Both workspaces coexist in one Foundry instance with real members, teams,
  projects, issues, and admin credentials. On the htmx web tier, every read and
  write Marco makes is scoped to Acme; a crafted or stale link to a Globex
  resource is refused identically to a never-existed one; and Priya cannot manage
  Globex's credentials while acting on Acme. A contractor who belongs to BOTH
  workspaces acts on exactly one at a time and switching changes which tenant's
  data the web shows. Proven with REAL Acme/Globex fixtures, not synthetic ids.

  Background:
    Given workspace "Acme" exists with admin "priya@acme.com"
    And workspace "Globex" exists with admin "olivia@globex.com"
    And "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And the "Globex" project "Core" has issues GLOBEX-1 and GLOBEX-2

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able isolation proof on the web read path.
  #    "A member of Acme, browsing the web, sees only Acme's board and issues."
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e
  Scenario: A member sees only their own workspace's board on the web
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens the "Acme" project "Auth" board on the web
    Then only "Acme" data appears on the web
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 2. Read isolation — issue detail is scoped too (a second representative read).
  # ----------------------------------------------------------------------------
  Scenario: A member reads only their own workspace's issue detail on the web
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens issue ACME-1 in the "Acme" project "Auth" on the web
    Then only "Acme" data appears on the web
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 3. Write isolation — a write affects ONLY the acting workspace.
  # ----------------------------------------------------------------------------
  Scenario: A member's write affects only their own workspace on the web
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member files issue "Rotate signing keys" in the "Acme" project "Auth" on the web
    Then the new issue appears only in "Acme" on the web
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 4. Non-enumerable refusal (evil-user, the security core) — a crafted link to a
  #    FOREIGN board is refused IDENTICALLY to a never-existed board.
  # ----------------------------------------------------------------------------
  @error
  Scenario: Reaching another workspace's board by its real address is refused non-enumerably
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens the "Globex" project "Core" board on the web by its real address
    And the member opens a project board that never existed on the web
    Then the two web responses are refused identically
    And nothing on the web reveals the "Globex" board exists

  # ----------------------------------------------------------------------------
  # 5. Non-enumerable refusal (evil-user) — a crafted link to a FOREIGN issue is
  #    refused identically to a never-existed issue.
  # ----------------------------------------------------------------------------
  @error
  Scenario: Reaching another workspace's issue by its real address is refused non-enumerably
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens issue GLOBEX-1 in the "Globex" project "Core" on the web
    And the member opens an issue that never existed on the web
    Then the two web responses are refused identically
    And nothing on the web reveals the "Globex" issue exists

  # ----------------------------------------------------------------------------
  # 6. Non-enumerable WRITE refusal (evil-user) — a crafted POST into a FOREIGN
  #    project is refused identically to a write into a never-existed project, and
  #    no Globex row is created.
  # ----------------------------------------------------------------------------
  @error
  Scenario: Writing into another workspace's project is refused non-enumerably
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member files issue "Sneaky" in the "Globex" project "Core" on the web
    And the member files issue "Sneaky" in a project that never existed on the web
    Then the two web responses are refused identically
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 7. Admin authority does not cross tenants (evil-user) — an Acme admin acting on
  #    Acme cannot revoke a Globex credential; it is refused non-enumerably and the
  #    Globex credential is unchanged.
  # ----------------------------------------------------------------------------
  @error
  Scenario: An admin of one workspace cannot manage another's credentials on the web
    Given "priya@acme.com" is signed in on the web acting on workspace "Acme"
    And the "Globex" workspace has an admin credential "globex-ci"
    When the "Acme" admin tries to revoke the "Globex" credential "globex-ci" on the web
    Then the web request is refused identically to a never-existed credential
    And no "Globex" membership or credential is changed

  # ----------------------------------------------------------------------------
  # 8. Multi-membership (OD-2 / ADR-005) — a contractor in BOTH workspaces acts on
  #    exactly the workspace their session is resolved to.
  # ----------------------------------------------------------------------------
  Scenario: A multi-membership user acts on exactly their active workspace on the web
    Given "dana@contract.dev" is also a member of "Acme" in team "Backend" with project "Auth" prefix "ACME"
    And "dana@contract.dev" is also a member of "Globex" in team "Platform" with project "Core" prefix "GLOBEX"
    And "dana@contract.dev" is signed in on the web acting on workspace "Acme"
    When the member opens the "Acme" project "Auth" board on the web
    Then only "Acme" data appears on the web
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 9. Multi-membership switch — switching the active workspace changes which
  #    tenant's data the web shows.
  # ----------------------------------------------------------------------------
  Scenario: Switching the active workspace changes which workspace's data is shown
    Given "dana@contract.dev" is also a member of "Acme" in team "Backend" with project "Auth" prefix "ACME"
    And "dana@contract.dev" is also a member of "Globex" in team "Platform" with project "Core" prefix "GLOBEX"
    And "dana@contract.dev" is signed in on the web acting on workspace "Acme"
    When the member switches their active workspace to "Globex"
    And the member opens the "Globex" project "Core" board on the web
    Then only "Globex" data appears on the web
    And no "Acme" data appears on the web
