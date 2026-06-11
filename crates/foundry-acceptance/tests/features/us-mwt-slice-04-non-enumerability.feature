# Feature: multi-workspace-tenancy — Slice 4: cross-tenant NON-ENUMERABILITY
# HARDENING. Slices 2-3 proved the isolation boundary PER SURFACE (web tier;
# JSON /api/v1 + machine-token + session resolution). This slice does NOT add a
# new surface — it UNIFIES and PROVES, with an explicit ADVERSARIAL MATRIX, that
# the cross-tenant refusal is OBSERVATIONALLY IDENTICAL to a never-existed id on
# EVERY tenant-scoped surface: same status, same body, no 403-vs-404 oracle, no
# id/slug echo, no shape difference (US-MWT05 / NFR-MWT-SEC-02 / ADR-003).
#
# Hypothesis (slices/slice-04-non-enumerability-hardening.md): NO surface has an
# existence oracle — every "not yours" collapses to "doesn't exist" identically.
# Disproved if ANY surface's refusal differs (status, body, or shape) in a way
# that confirms a foreign resource exists.
#
# What slice 4 adds OVER slices 2-3: slices 2-3 each proved the refusal core on
# the HIGHEST-TRAFFIC representative path of their surface (web board/issue/file
# + admin revoke; API issue read/write + token list/revoke). Slice 4's job is
# the COMPLETENESS sweep — the remaining tenant-scoped web/API surfaces that the
# representative proofs did not individually exercise (web issue-detail + the
# state-change/comment/attachment writes; the API state-change PATCH + comment
# create) — each asserting foreign-id ≡ never-existed-id, PLUS the cross-surface
# ORACLE-HUNT invariants (no 403 anywhere; no id/slug echo in any refusal body).
#
# Driving adapters (LAYER 3, @real-io): the htmx web tier (foundry-app over real
# HTTP, real signed-in foundry_session cookie + double-submit CSRF) AND the JSON
# /api/v1 (foundry-api over real HTTP, real EdDSA MachinePrincipal bearer bound
# by token.workspace_id). Tenant-scoped surfaces under test:
#   Web  GET  /team/{t}/project/{p}/issues/{n}                  (issue detail)
#   Web  POST /team/{t}/project/{p}/issues/{n}/comments         (comment write)
#   Web  POST /team/{t}/project/{p}/issues/{n}/state            (state change)
#   Web  POST /team/{t}/project/{p}/issues/{n}/attachments      (upload)
#   Web  GET  /team/{t}/project/{p}/issues/{n}/attachments/{id} (download)
#   API  PATCH /api/v1/teams/{t}/projects/{p}/issues/{n}        (state change)
#   API  POST  /api/v1/teams/{t}/projects/{p}/issues/{n}/comments (comment)
# Each is reached for a REAL Globex resource (foreign) and for a never-existed
# id, and the two responses are asserted byte-identical.
#
# Refusal-status contract (ADR-003 / OD-MWT-D6, RESOLVED in slices 2-3 and
# carried here): every CROSS-tenant resource reach is the SAME 404 (web page /
# API JSON envelope) as a never-existed id — generalising the shipped
# `find_*_in_workspace → None` idiom (and `find_attachment_for_requester` for
# the attachment surface). Cross-tenant access NEVER 403s (a 403-vs-404
# difference is an existence oracle). Intra-workspace authz failures keep their
# shipped shape (ADR-003 boundary clause) and are OUT of this matrix.
#
# Timing oracle (ADR-003): the foreign-id and missing-id paths execute the SAME
# `WHERE id AND workspace_id` query, so they share a timing profile BY
# CONSTRUCTION. This slice asserts that STRUCTURALLY (status + body identity =>
# the same None path was taken) — NOT by flaky wall-clock measurement. A genuine
# constant-time concern, if any, is documented as a residual in the wave-decisions
# doc, not asserted by a timing scenario (which would be flaky under @all load).
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (no
# import/collection error => not BROKEN). At runtime against the real
# testcontainers PG16 the genuine RED is MISSING_FUNCTIONALITY:
#   1. The Background seeds Acme then Globex; the SECOND `INSERT INTO workspaces`
#      FAILS on `uniq_one_workspace` (0001_init.sql:15) until DELIVER ships
#      `0002_multi_workspace.sql` (shared with slices 1-3).
#   2. Once two workspaces coexist, the web session's acting workspace resolution
#      (ADR-005 membership resolution, shipped by slices 1-2 DELIVER) scopes the
#      member to Acme; every cross-tenant reach then collapses to the shipped
#      `find_*_in_workspace → None` 404. The matrix PROVES the shipped scoping is
#      uniform across the remaining surfaces under a genuinely-coexisting second
#      workspace — green-by-inheritance behind the `0002` gate. No new production
#      module is introduced by this slice; a matrix cell that reds for a REAL
#      oracle (a 403, a body echo, a shape diff) is flagged in
#      distill/slice-04-upstream-issues.md.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter
# scenarios are example-based (NOT property-based); every adversarial path is
# enumerated explicitly; no PBT machinery at this layer. Mandate 8 state-delta is
# layers 1-3; at layer 3 traditional assertions over port-exposed observables
# (HTTP refusal status + byte-identical body; post-write workspace-scoped DB row
# presence proving no foreign mutation) are used per the Layered Test Discipline
# table (matching slices 1-3; no `state_delta.rs` Rust port exists — Python is
# the canonical pilot).
#
# Scope: SLICE 4 ONLY — the cross-tenant non-enumerability HARDENING + the
# adversarial matrix across ALL surfaces (US-MWT05). Migration-as-guarantee
# (Slice 5) and provisioning (Slice 6) are explicitly OUT — do not add them here.
# Slices 1-3 (coexistence; web tier; API/token/session) are NOT re-authored;
# scenarios 1-3 + 8 + 11 below INCLUDE the slice-2/3 surfaces for regression /
# completeness (one consolidated matrix), reusing their registered step text.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-
# time DELIVER cycle; DELIVER unskips one scenario per RED->GREEN->COMMIT cycle).

@multi-workspace-tenancy @mwt-slice-04 @real-io @driving_adapter @us-mwt05
Feature: Another tenant's existence and resources are invisible from my workspace on every surface
  Marco is a member of "Acme"; "Globex" coexists in the same Foundry instance
  with real members, teams, projects, issues, comments, attachments, and admin
  credentials. On EVERY tenant-scoped surface — web reads, web writes (comment,
  state change, attachment), web admin actions, the JSON /api/v1 reads and writes
  and token revoke — a request Marco makes for a real Globex resource is
  refused IDENTICALLY to a request for an id that never existed: the same status,
  the same body, and never a 403 that would distinguish "exists but forbidden"
  from "does not exist". No refusal body echoes the foreign id or slug. The
  boundary is non-enumerable everywhere. Proven with REAL Acme/Globex fixtures.

  Background:
    Given workspace "Acme" exists with admin "priya@acme.com"
    And workspace "Globex" exists with admin "olivia@globex.com"
    And "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And the "Globex" project "Core" has issues GLOBEX-1 and GLOBEX-2

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able non-enumerability proof. "A member of
  #    Acme reaching a real Globex issue on the web is indistinguishable from
  #    reaching an issue that never existed." (The single load-bearing claim of
  #    the whole slice, on the highest-traffic web read.)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @error
  Scenario: A foreign web issue and a never-existed web issue are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens issue GLOBEX-1 in the "Globex" project "Core" on the web
    And the member opens an issue that never existed on the web
    Then the two web responses are refused identically
    And the web refusal reveals no foreign identifier

  # ----------------------------------------------------------------------------
  # 2. Web board read — foreign board reach is indistinguishable from a board
  #    that never existed (completeness alongside the issue-detail proof).
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web board and a never-existed web board are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member opens the "Globex" project "Core" board on the web by its real address
    And the member opens a project board that never existed on the web
    Then the two web responses are refused identically
    And the web refusal reveals no foreign identifier

  # ----------------------------------------------------------------------------
  # 3. Web file-issue WRITE — a foreign-project write is refused identically to a
  #    never-existed-project write, and no Globex row is created.
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web issue-create and a never-existed web issue-create are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member files issue "Sneaky" in the "Globex" project "Core" on the web
    And the member files issue "Sneaky" in a project that never existed on the web
    Then the two web responses are refused identically
    And no "Globex" data appears on the web

  # ----------------------------------------------------------------------------
  # 4. Web comment WRITE — posting a comment onto a real Globex issue is refused
  #    identically to commenting on an issue that never existed; no Globex
  #    comment is created. (A web write surface slices 2-3 did not individually
  #    exercise — shares the `find_team_by_slug(ws,..) -> find_project_by_slug`
  #    scoping chain.)
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web comment and a never-existed web comment are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member comments on issue GLOBEX-1 in the "Globex" project "Core" on the web
    And the member comments on an issue that never existed on the web
    Then the two web responses are refused identically
    And no comment was created in "Globex"

  # ----------------------------------------------------------------------------
  # 5. Web state-change WRITE — changing the state of a real Globex issue is
  #    refused identically to changing an issue that never existed; the Globex
  #    issue's state is unchanged.
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web state-change and a never-existed web state-change are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member changes the state of issue GLOBEX-1 in the "Globex" project "Core" on the web
    And the member changes the state of an issue that never existed on the web
    Then the two web responses are refused identically
    And no "Globex" issue changed state

  # ----------------------------------------------------------------------------
  # 6. Web attachment UPLOAD WRITE — uploading onto a real Globex issue is
  #    refused identically to uploading onto an issue that never existed; no
  #    Globex attachment is created.
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web attachment-upload and a never-existed upload are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    When the member uploads a file to issue GLOBEX-1 in the "Globex" project "Core" on the web
    And the member uploads a file to an issue that never existed on the web
    Then the two web responses are refused identically
    And no attachment was created in "Globex"

  # ----------------------------------------------------------------------------
  # 7. Web attachment DOWNLOAD read — downloading a real Globex attachment (the
  #    canonical `find_attachment_for_requester` idiom) is refused identically to
  #    a never-existed attachment id; nothing reveals the Globex attachment is
  #    real.
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web attachment-download and a never-existed download are indistinguishable
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    And the "Globex" project "Core" issue GLOBEX-1 has an attachment
    When the member downloads the "Globex" attachment on the web
    And the member downloads an attachment that never existed on the web
    Then the two web responses are refused identically
    And the web refusal reveals no foreign identifier

  # ----------------------------------------------------------------------------
  # 8. Web admin action — an Acme admin reaching a Globex credential is refused
  #    identically to a never-existed credential (cross-tenant authz collapses to
  #    the SAME non-enumerable 404, admin_tokens.rs:48; carried from slice 2 into
  #    the unified matrix to assert NO 403 oracle here).
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign web admin revoke and a never-existed revoke are indistinguishable
    Given "priya@acme.com" is signed in on the web acting on workspace "Acme"
    And the "Globex" workspace has an admin credential "globex-ci"
    When the "Acme" admin tries to revoke the "Globex" credential "globex-ci" on the web
    And the "Acme" admin tries to revoke a credential that never existed on the web
    Then the two web responses are refused identically
    And no "Globex" membership or credential is changed

  # ----------------------------------------------------------------------------
  # 9. API state-change PATCH WRITE — an Acme token patching a real Globex issue
  #    is refused identically to patching an issue that never existed; the Globex
  #    issue is unchanged. (An /api/v1 write surface slices 1-3 did not
  #    individually exercise.)
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign API state-change and a never-existed state-change are indistinguishable
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential changes the state of issue GLOBEX-1 in the "Core" project over the API by its real address
    And the Acme-bound credential changes the state of an issue that never existed over the API
    Then the two API responses are refused identically
    And the API refusal reveals no foreign identifier

  # ----------------------------------------------------------------------------
  # 10. API comment WRITE — an Acme token commenting on a real Globex issue is
  #     refused identically to commenting on an issue that never existed; no
  #     Globex comment is created. (An /api/v1 write surface slices 1-3 did not
  #     individually exercise.)
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign API comment and a never-existed API comment are indistinguishable
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential comments on issue GLOBEX-1 in the "Core" project over the API by its real address
    And the Acme-bound credential comments on an issue that never existed over the API
    Then the two API responses are refused identically
    And no comment was created in "Globex"

  # ----------------------------------------------------------------------------
  # 11. API token REVOKE — an Acme token revoking a real Globex jti is refused as
  #     not-found identically to a never-existed jti, and the Globex token stays
  #     active. (Carried from slice 3 into the unified matrix; the token surface
  #     completeness cell + the oracle-hunt no-echo assertion.)
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: A foreign API token revoke and a never-existed revoke are indistinguishable
    Given a machine credential is bound to "ops@acme.com" in workspace "Acme"
    And a managed token "globex-ci" exists in workspace "Globex"
    When the Acme-bound credential revokes the "Globex" token "globex-ci" over the API
    And the Acme-bound credential revokes a token id that exists nowhere over the API
    Then the two API revoke responses are refused identically as not found
    And the "Globex" token "globex-ci" remains active

  # ----------------------------------------------------------------------------
  # 12. ORACLE HUNT — no refusal anywhere in the matrix is a 403. Across the web
  #     and API foreign reaches gathered in this scenario, every cross-tenant
  #     refusal is a 404 (never a 403 that would distinguish "exists but
  #     forbidden" from "does not exist"). This is the explicit no-403 assertion
  #     the matrix turns on.
  # ----------------------------------------------------------------------------
  @error @pending
  Scenario: No cross-tenant refusal anywhere is an existence-revealing 403
    Given "marco@acme.com" is signed in on the web acting on workspace "Acme"
    And a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the member probes the "Globex" issue GLOBEX-1 in project "Core" across the web and the API
    Then no cross-tenant refusal in this scenario is a 403
    And every cross-tenant refusal in this scenario is a non-enumerable 404
