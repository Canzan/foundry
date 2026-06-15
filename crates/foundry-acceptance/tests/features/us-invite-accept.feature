# Feature: invite-accept-flow — a provisioned first-admin CLAIMS THEIR ACCOUNT.
# This turns the emitted `/invites/accept?id=…&sig=…` link from a DEAD URL (today
# it is rendered by three provision emit sites — admin_cli.rs:505,
# instance_admin.rs:272, bootstrap.rs:270 — but no route, no consume fn, and no
# password-set handler exist) into a real claim-your-account vertical: verify the
# signed token → render set-password → atomically consume the invite (single-use)
# + write the password in ONE tx → auto sign-in → land on the workspace. This is
# the web-provisioning-flow ADR-005 / D5 deferred follow-up, the single
# highest-value cut left out of provisioning v1. Scope (v1, user-ratified):
# FIRST-ADMIN invites ONLY.
#
# Hypothesis (design/architecture.md C4 + GET/POST flows; design/wave-decisions.md
# D1-D7, RATIFIED OD table 2026-06-14): a provisioned first-admin holding a live
# signed invite link can verify it, set her own password, be atomically signed in,
# and land ON her workspace seeing only that tenant — while EVERY invalid-link
# reason (expired / already-used / tampered-signature / unknown-id) collapses to
# ONE byte-identical refusal (status + body), the consume is single-use under
# concurrency, the public POST is CSRF-guarded, no secret leaks to logs, and a
# password mistake is recoverable inline WITHOUT consuming the invite. We KNOW we
# are right when: a valid GET renders the set-password form naming the workspace
# and mutates nothing; a valid POST consumes the invite exactly once, writes the
# argon2id hash, establishes a session, and lands her on `invites.workspace_id`
# with no separate login step; all four refusal arms are byte-identical; two
# concurrent accepts of one live invite succeed exactly once; a CSRF-less POST is
# refused before any consume; logs carry no `sig` and no password; and a weak /
# mismatched password re-renders inline with the invite still live and re-usable.
# DISPROVED if: GET mutates state; any refusal arm diverges in status OR body; a
# consumed/expired link is accepted; both concurrent accepts win (or neither); a
# CSRF-less POST consumes or writes; a `sig` or password reaches the logs; or a
# rejected-password attempt consumes the invite (stranding the admin on a dead link).
#
# Driving adapter (LAYER 3, @real-io): the PUBLIC (signed-out-accessible)
# `/invites/accept` GET+POST route pair served by foundry-app over real HTTP — the
# same in-process axum router / InProcHarness (`foundry_app::test_support::spawn_app`)
# the Feature-B + slice-02/04 + web-provisioning web scenarios drive — mounted on
# the PUBLIC layer of `build_router` (NOT behind the instance-admin gate; the
# invitee is not signed in yet) UNDER the SHIPPED session + double-submit CSRF
# layers. Two new handlers (D1, architecture.md C4-L3):
#   GET  /invites/accept?id=<uuid>&sig=<urlencoded-hmac>  (verify → render form OR uniform refusal; NON-COMMITTAL; mints CSRF cookie)
#   POST /invites/accept  (id + sig + password + confirm + _csrf → consume+write+session → 303 / OR inline error OR uniform refusal OR 403)
#
# Driven adapters exercised (LAYER 3, @real-io) — NO MOCKS at the acceptance level:
#   * real Postgres (the `invites` row with the SHIPPED `used_at`/`used_by`
#     single-use columns — 0001_init.sql:99-100, NO migration per D1/adr-001;
#     `users`, `workspaces`, `workspace_memberships`, `tower_sessions`) via
#     testcontainers PG16 + per-scenario schema;
#   * the real tower-sessions Postgres store (the auto-sign-in session);
#   * the real double-submit CSRF middleware (`csrf_middleware`) + `ensure_csrf_cookie`
#     minted on the GET (D4/adr-003 — the public-route CSRF seam, signed-out, like sign-in);
#   * the SHIPPED `InviteToken::verify` (HMAC over invite_id‖expires_at — the tamper
#     oracle, rejects tampered/extended links with no DB hit);
#   * the SHIPPED `hash_password` (argon2id, OWASP, on spawn_blocking);
#   * the NEW `Store::set_first_admin_password_and_consume` one-TX guarded-UPDATE
#     (mirrors the SHIPPED `claim_bootstrap_token` idiom — D1/D2/adr-001);
#   * the NEW tiny `foundry_auth::check_password_policy` (min-12, length-first, NIST
#     800-63B — D5/adr-004, applied BEFORE the consume TX opens);
#   * the SHIPPED `resolve_active_workspace` membership seam (the landing tenant).
# The Background seeds a REAL invite by running the SHIPPED provisioning path (the
# emit site that mints the live signed token + the `invites` row), so the token
# under test is genuine — not synthesised.
#
# Refusal / non-enumerability decision (D3/adr-002, the SECURITY CRUX): every
# invalid-link reason — expired, already-used, tampered-signature, unknown-id —
# returns ONE `invite_refusal_page()` that is BYTE-IDENTICAL (status AND full body)
# across all four. OD-3 RATIFIED 2026-06-14: the fixed status is 200 OK (avoids
# even a status-code oracle; honest "this page exists, the link is dead" UX). The
# refusal leaks NONE of workspace name, account existence, or invite state; the
# reason lives ONLY in internal `tracing` keyed on `invite_id` (NFR-3, NFR-5). A
# VALID invite renders the set-password form — that is NOT an oracle: the holder of
# a valid signed token already knows it is valid. The byte-identity bar is asserted
# on the FULL response (status + body bytes), never merely same-status (the slice-04
# lesson: same-status hid 4 oracles). A revert-reds-it litmus binds it: collapsing
# any two refusal arms into divergent responses MUST re-RED the byte-identity
# assertion.
#
# Security divergence (deliberate, recorded in design/upstream-changes.md Finding 2):
# the SHIPPED bootstrap claim flow (`bootstrap.rs:124-139`) returns DISTINCT "Link
# already used" / "Link expired" / "Link not found" messages at 410 — an ENUMERATION
# ORACLE. This feature MUST NOT replicate that. The accept flow deliberately
# DIVERGES toward the GOOD `resource_not_found_page()` uniform-refusal posture
# (`bootstrap.rs:340`). Bootstrap is NOT modified here (it is an out-of-scope
# security follow-up).
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES — this file
# is Gherkin text, it adds NO undefined-symbol reference to any `.rs`, and it does
# NOT edit `acceptance.rs` (so `inventory` force-linking is untouched) → NOT BROKEN.
# Cucumber-rs leaves unmatched step text as a runtime skip, not a compile error
# (the same RED-state contract us-mwt-web-provisioning.feature relies on). Genuine
# RED is MISSING_FUNCTIONALITY at runtime against the real testcontainers PG16:
#   1. The `invites_accept.rs` web adapter, the GET `show_accept_form` + POST
#      `submit_accept` handlers, the two `.route("/invites/accept", get().post())`
#      lines on the PUBLIC layer of `build_router`, and the set-password +
#      uniform-refusal Askama templates DO NOT EXIST YET — every scenario fails
#      because the route is unknown / the handler is absent.
#   2. `Store::consume_invite` + `Store::set_first_admin_password_and_consume` DO
#      NOT EXIST YET — even once a route exists, the consume + one-TX password write
#      is missing (the genuinely-new backend, D1/D2). No migration is owed (the
#      `used_at`/`used_by` columns shipped in 0001 — headline finding).
#   3. `foundry_auth::check_password_policy` DOES NOT EXIST YET — the inline weak /
#      mismatch recovery path (US-03) has no policy to enforce.
# A scenario that reds for a REAL oracle (a divergent refusal arm, a status/body
# leak, a double-consume, a CSRF bypass, an invite consumed on a rejected password)
# is flagged in distill/upstream-issues.md, not silently accepted.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter scenarios
# are EXAMPLE-BASED (NOT property-based); every sad / evil-user / adversarial path
# is enumerated explicitly; NO PBT machinery at this layer. The `@property`-tagged
# scenarios (non-enumerability, single-use-under-concurrency, no-secret-leakage)
# remain EXAMPLE-PINNED at layer 3 (matching the journey feature + slice-04
# convention), with their universal-invariant SHAPE preserved in the title for the
# DELIVER crafter. Mandate 8 state-delta is a layers-1-3 Python pilot; NO
# `state_delta.rs` Rust port exists (matching slices 1-6 + web-provisioning), so
# LAYER-3 assertions are traditional assertions over port-exposed web observables:
# rendered page/fragment substrings (workspace name, set-password form, inline
# error), HTTP refusal status + BYTE-IDENTICAL refusal body, redirect-and-landed
# tenant, post-consume `invites.used_at` set exactly once, post-error invite still
# live, and a log scan free of `sig`/password.
#
# Scope: the `/invites/accept` GET+POST claim-your-account vertical for FIRST-ADMIN
# invites ONLY (US-01/02/03). General workspace-member invites (a later feature), a
# CLI-native `foundry invite accept` TUI (the link is a web URL — fixing the web
# route fixes both emit sites, journey CLI-parity note), and any change to the
# bootstrap claim flow's enumeration oracle (out-of-scope security follow-up) are
# explicitly OUT — do not add them here.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time
# DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle; @pending
# is excluded by the harness default + @all lanes per acceptance.rs).

@invite-accept-flow @real-io @driving_adapter
Feature: A provisioned first-admin claims her account from her invite link and is signed in
  Priya Nair was just provisioned the "Northwind" workspace by her instance
  super-admin, who pasted her an invite link valid for 7 days. Her admin account
  was created with a password hash she has never seen — this link is the ONLY way
  she can establish a credential and get in. Today the link is a dead URL. With
  this flow she opens it, sees a set-password form naming "Northwind", chooses her
  own password, submits once, and lands straight on her workspace dashboard signed
  in — no separate login step, seeing only her tenant. Every bad link — expired,
  already-used, tampered, or unknown — shows ONE calm, byte-identical "this invite
  is no longer valid" page that reveals nothing about whether any account or
  workspace exists. A link is consumable exactly once, even under a double-click
  race. The state-changing submit is forgery-protected. A password typo is fixed
  inline without burning the invite. Proven with a REAL invite minted by the
  shipped provisioning path over the real session + CSRF + Postgres machinery — no
  mocks.

  Background:
    Given a super-admin provisioned the "Northwind" workspace
    And Priya Nair was seeded as its first-admin with a live invite link valid for 7 days

  # ----------------------------------------------------------------------------
  # 1. WALKING SKELETON — the demo-able headline value, end-to-end through the new
  #    public web driving adapter. "A first-admin with a live invite opens the
  #    accept page, sets a valid password, and lands on her workspace signed in,
  #    seeing only her tenant." The thinnest cut that proves the NEW public route
  #    wires through the session + CSRF layers to the NEW one-TX consume+write and
  #    back to an auto-signed-in landing. (US-01: AC-01.1/01.3/01.4/01.5.)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-01
  Scenario: A first-admin sets her password and lands on her workspace signed in
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    When she sets a password meeting the strength policy and confirms it
    Then she is signed in without a separate login step
    And she lands on the "Northwind" workspace dashboard
    And she sees no data from any other workspace
    And her invite is recorded as used exactly once

  # ----------------------------------------------------------------------------
  # 2. The GET accept page renders the set-password form for a live invite, naming
  #    the workspace for context (US-01: AC-01.1). The arrival step of the chained
  #    journey — its Given is reused as the precondition of scenarios 3, 11, 12.
  # ----------------------------------------------------------------------------
  @us-01
  Scenario: A live invite renders a set-password form naming the workspace
    Given Priya's invite has not expired and has not been used
    When Priya opens her invite link
    Then she sees a set-password form
    And the form names the "Northwind" workspace

  # ----------------------------------------------------------------------------
  # 3. The GET is NON-COMMITTAL — opening the accept page mutates no state (the
  #    invite stays unconsumed; no password is written). The TOCTOU-safety
  #    foundation: only the POST consume TX is authoritative (D6/AC-01.2). Its
  #    Given reuses scenario 2's Given+When (chained narrative, Pillar 2).
  # ----------------------------------------------------------------------------
  @us-01
  Scenario: Opening the accept page consumes nothing
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    Then no password has yet been set on her account
    And her invite is still live and unconsumed

  # ----------------------------------------------------------------------------
  # 4. Boundary — an invite opened just INSIDE its expiry window (issued 6 days 23
  #    hours ago, i.e. expires_at - 1s) still renders and accepts (NFR-1/AC-01.6).
  # ----------------------------------------------------------------------------
  @us-01
  Scenario: An invite opened just inside its expiry window is accepted
    Given Priya's invite is one second away from expiring and has not been used
    When Priya opens her invite link and sets a valid password
    Then she is signed in on the "Northwind" workspace

  # ----------------------------------------------------------------------------
  # 5. SECURITY (US-02) — an EXPIRED link is refused without leaking existence. The
  #    canonical refusal arm; scenarios 6/7/8 assert byte-identity AGAINST this one.
  #    (E1; AC-02.3, NFR-3.)
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: An expired invite is refused without leaking existence
    Given Priya's invite expired one day ago
    When Priya opens her invite link
    Then she sees the standard "invite is no longer valid" page
    And the page reveals nothing about whether any account or workspace exists
    And the page advises asking the instance administrator to re-issue the invite

  # ----------------------------------------------------------------------------
  # 6. Boundary — a link opened just OUTSIDE its expiry window (expires_at + 1s) is
  #    refused with that same uniform page (NFR-1/AC-02.4). Pairs with scenario 4.
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: An invite opened just past its expiry window is refused
    Given Priya's invite expired one second ago
    When Priya opens her invite link
    Then she sees the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal

  # ----------------------------------------------------------------------------
  # 7. SECURITY (US-02) — a TAMPERED signature is refused IDENTICALLY to an expired
  #    link. The HMAC tamper oracle fails before any DB hit (D6); the user-visible
  #    response must be byte-identical (status + full body) to scenario 5. (E3;
  #    AC-02.1/02.2, NFR-3.)
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: A tampered signature is refused identically to an expired link
    Given Priya's invite is live but the signature in the link has been altered by one character
    When Priya opens the tampered link
    Then she sees the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal

  # ----------------------------------------------------------------------------
  # 8. SECURITY (US-02) — an UNKNOWN invite id is refused IDENTICALLY to every other
  #    reason. A prober cannot tell the id never existed. (E4; AC-02.1/02.2, NFR-3.)
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: An unknown invite id is refused identically to every other reason
    Given an invite id that was never issued
    When someone opens an accept link with that id
    Then they see the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal
    And nothing reveals whether that id, account, or workspace exists

  # ----------------------------------------------------------------------------
  # 9. SECURITY (US-02) @property — NON-ENUMERABILITY: the four invalid reasons
  #    {expired, already-used, tampered-signature, unknown-id} ALL produce a
  #    byte-identical user-visible refusal (status + full body); they differ ONLY in
  #    internal logging, never in the observable response. The revert-reds-it litmus
  #    binds it: collapsing any two arms into divergent responses re-REDs this.
  #    Example-pinned at LAYER 3 (Mandate 11). (AC-02.1/02.2, NFR-3, the security crux.)
  # ----------------------------------------------------------------------------
  @us-02 @error @property
  Scenario: Invalid-link refusals are byte-identical across all four reasons
    Given an expired invite, an already-used invite, a tampered-signature link, and an unknown-id link
    When each is opened
    Then all four produce a byte-identical user-visible refusal page
    And they differ only in internal logging, never in the observable response

  # ----------------------------------------------------------------------------
  # 10. SECURITY (US-02) — SINGLE-USE: a consumed invite re-opened is refused; no
  #     new password is set and no session is created. Proves exactly-once at the
  #     handler level (E2; AC-02.5, NFR-2). Its Given reuses the walking skeleton's
  #     successful accept (chained narrative).
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: A consumed invite can never be used again
    Given Priya has already set her password and signed in via her invite link
    When Priya opens the same invite link again
    Then she sees the standard "invite is no longer valid" page
    And no new password is set and no session is created

  # ----------------------------------------------------------------------------
  # 11. SECURITY (US-02) @property — SINGLE-USE UNDER CONCURRENCY: two accept
  #     submissions for one live invite arrive concurrently; the guarded-UPDATE
  #     (not a read-then-write) means exactly one consumes + signs in, the other
  #     gets the uniform refusal, and `invites.used_at` is set exactly once. The
  #     race oracle for NFR-2. Example-pinned at LAYER 3 (Mandate 11). (E7; AC-02.6.)
  # ----------------------------------------------------------------------------
  @us-02 @error @property
  Scenario: Concurrent accepts of one invite succeed exactly once
    Given Priya's invite is live
    When two accept submissions for the same invite arrive concurrently
    Then exactly one submission sets the password and signs in
    And the other receives the standard "invite is no longer valid" page
    And the invite is recorded as used exactly once

  # ----------------------------------------------------------------------------
  # 12. SECURITY (US-02) — CSRF: the public state-changing POST without a valid
  #     double-submit token is refused by the SHIPPED csrf_middleware BEFORE the
  #     handler runs — no invite is consumed and no password is written (E8;
  #     AC-02.8, NFR-6). The cookie is minted on the GET (D4/adr-003).
  # ----------------------------------------------------------------------------
  @us-02 @error
  Scenario: An accept submission without a valid security token is refused
    Given a forged accept submission for a live invite without a valid security token
    When it reaches the accept endpoint
    Then it is refused by the request-forgery protection
    And no invite is consumed and no password is written

  # ----------------------------------------------------------------------------
  # 13. SECURITY (US-02) @property — NO-SECRET-LEAKAGE: across a full accept +
  #     refusal cycle, the application logs contain neither the invite `sig` value
  #     nor any submitted password (the reason for a refusal lives in tracing keyed
  #     on invite_id only). Example-pinned at LAYER 3 (Mandate 11). (AC-02.9, NFR-5.)
  # ----------------------------------------------------------------------------
  @us-02 @error @property
  Scenario: No invite signature or password ever appears in the logs
    Given Priya completes a successful accept and a hostile prober is refused
    When the application logs for the full cycle are examined
    Then no invite signature value appears in the logs
    And no submitted password appears in the logs

  # ----------------------------------------------------------------------------
  # 14. SECURITY (US-02) — TOCTOU: a link consumed in the GET→POST window is refused
  #     by the consume TX guard (the GET liveness check is advisory only); expiry is
  #     enforced INSIDE the TX, not just on GET (D6; AC-02.7, NFR-1/NFR-2).
  # ----------------------------------------------------------------------------
  @pending @us-02 @error
  Scenario: A link consumed between opening the page and submitting is refused by the transaction guard
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    And the same invite is consumed by another submission before Priya submits
    When Priya submits a valid password on her now-stale page
    Then she sees the standard "invite is no longer valid" page
    And no second password write occurs and the invite stays used exactly once

  # ----------------------------------------------------------------------------
  # 15. RECOVERY (US-03) — a WEAK password (below the min-12 policy) is corrected
  #     inline; the policy check runs BEFORE the consume TX opens, so the invite is
  #     NOT consumed and stays live; no session is created (E5; AC-03.1, FR-5/NFR-4).
  # ----------------------------------------------------------------------------
  @pending @us-03 @error
  Scenario: A weak password is corrected inline and the invite stays live
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    When she submits a password below the strength policy
    Then she sees an inline error explaining the minimum password length
    And her invite is still live and unconsumed
    And no session is created

  # ----------------------------------------------------------------------------
  # 16. RECOVERY (US-03) — a MISMATCHED confirmation is corrected inline; the invite
  #     is NOT consumed and stays live (E6; AC-03.2, FR-5).
  # ----------------------------------------------------------------------------
  @pending @us-03 @error
  Scenario: A mismatched confirmation is corrected inline and the invite stays live
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    When her confirmation does not match her new password
    Then she sees an inline error that the passwords do not match
    And her invite is still live and unconsumed

  # ----------------------------------------------------------------------------
  # 17. RECOVERY (US-03) — RE-ATTEMPT: after an inline error, re-submitting a VALID
  #     password on the SAME live invite completes the accept (the recoverability
  #     proof; AC-03.4, FR-5). Its Given chains off scenario 15's left-live invite.
  # ----------------------------------------------------------------------------
  @pending @us-03
  Scenario: A valid retry on the same invite after an error completes the accept
    Given Priya was shown an inline password error and her invite is still live
    When she submits a valid password on the same invite and confirms it
    Then she is signed in on the "Northwind" workspace
    And the invite is recorded as used exactly once

  # ----------------------------------------------------------------------------
  # 18. RECOVERY (US-03) — BOUNDARY: a password EXACTLY at the minimum length (12
  #     characters) is accepted (the policy is "at least 12"; AC-03.3, NFR-4).
  # ----------------------------------------------------------------------------
  @pending @us-03
  Scenario: A password exactly at the minimum length is accepted
    Given Priya has opened her live invite for "Northwind" and seen the set-password form
    When she submits a twelve-character password and confirms it
    Then her password is accepted and she is signed in on the "Northwind" workspace
