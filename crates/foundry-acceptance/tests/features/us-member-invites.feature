# Feature: workspace-member-invites — GENERALIZE the shipped first-admin
# `/invites/accept` flow to GENERAL workspace members. Two genuinely-new surfaces
# over an otherwise-reused vertical (see design/wave-decisions.md D1-D8, RATIFIED
# 2026-06-14):
#   (1) ISSUANCE — a workspace ADMIN invites a teammate by email at
#       `/workspace/invites` (a NEW admin-gated web driving adapter mirroring the
#       shipped `bootstrap::create_invite`: insert_invite + InviteToken::new + emit
#       link + best-effort email), non-enumerable 404 for non-admins/signed-out,
#       CSRF on the POST.
#   (2) ACCEPTANCE — the invitee (who has NO Foundry account) opens the link, sets a
#       password, and the accept POST runs ONE atomic tx (`create_member_and_consume`):
#       consume the invite + CREATE the user + ADD a `member`-role membership + write
#       the argon2id password, then auto-sign-in (303 -> the workspace dashboard).
# The accept route is ONE `/invites/accept` serving BOTH invite kinds via a
# data-derived DISPATCH (D3/adr-003): first-admin (the consumer IS `created_by`, a
# pre-existing user) -> the SHIPPED `set_first_admin_password_and_consume`; member
# (no user maps to `invitee_email`) -> the NEW `create_member_and_consume`. NO `kind`
# column, NO migration (D8/adr-004 — `invites.used_at`/`used_by`,
# `users.email_lower UNIQUE`, `workspace_memberships.role CHECK(admin|member)` all
# shipped in 0001_init.sql).
#
# Hypothesis (design/architecture.md C4-L1/L2/L3 + the four request flows;
# design/wave-decisions.md D1-D8 + the RESOLVED OD-A..OD-D / OD-1 table; discuss/
# requirements.md FR-1..8 + NFR-1..6 + BR-1..6; discuss/acceptance-criteria.md
# AC-01.1..04.5 + the four @property criteria): a workspace admin holding a session
# can issue a member invite that produces a working signed accept link; the invitee
# — with no prior account — can open that link, set a password, and in ONE atomic tx
# have an account created + a `member`-role membership added + the invite consumed +
# the password written, then be auto-signed-in onto `invites.workspace_id` seeing
# only that tenant, as a member (NOT an admin) — WHILE every non-admin/signed-out
# probe of `/workspace/invites` returns a 404 BYTE-IDENTICAL to a never-existed path
# (no oracle), every invalid accept reason {expired, already-used, tampered-sig,
# unknown-id, AND email-already-a-user} collapses to ONE byte-identical uniform
# refusal (status + full body, NEVER a 500), the invite creates exactly one account
# and is consumable exactly once even under a concurrent double-submit race, BOTH
# state-changing POSTs are CSRF-guarded, no `sig`/password reaches the logs, a
# password/email mistake is recoverable inline WITHOUT consuming the invite or
# creating an account, and the SHIPPED first-admin accept path still works (the kind
# dispatch did not break it).
# We KNOW we are right when: an admin POST creates an `invites` row (workspace =
# admin's active, invitee_email = typed, created_by = admin, expires_at = now + 7d)
# and renders the emitted `/invites/accept?id&sig` link (still rendered when the
# email send fails); a member-invite accept POST creates the user + a `member`
# membership + consumes + writes the hash in ONE tx, lands the new member on
# `invites.workspace_id` signed in with no second login, and that member 404s on
# `/workspace/invites`; the five refusal arms are byte-identical; two concurrent
# accepts of one live invite create the account EXACTLY once (one user, one
# membership, one consumed invite); an email-collision accept shows the uniform
# refusal (NOT a 500) with NO second account and the invite UNCONSUMED; both
# CSRF-less POSTs are refused before any write; logs carry no `sig` and no password;
# a weak/mismatched password and a blank issuance email re-render inline with NO
# side effect; and a first-admin invite still routes to the SHIPPED tx.
# DISPROVED if: a non-admin issues or even learns the issuance surface exists; a GET
# accept mutates state; any refusal arm diverges in status OR body, or the email
# collision surfaces as a 500/constraint-error page; an invite creates two accounts
# or is consumed twice under concurrency; a CSRF-less POST creates an invite, an
# account, or a consume; a `sig` or password reaches the logs; a rejected password
# consumes the invite or creates a half-formed account; the new member can reach
# `/workspace/invites`; or the SHIPPED first-admin accept path regresses.
#
# Driving adapters (LAYER 3, @real-io) — TWO surfaces over real HTTP, the same
# in-process axum router / InProcHarness (`foundry_app::test_support::spawn_app`) the
# Feature-B + slice-02/04 + web-provisioning + invite-accept scenarios drive, under
# the SHIPPED session + double-submit CSRF layers:
#   * ISSUANCE (NEW, admin-gated, mounted on the SHARED layer alongside
#     /admin/tokens + /workspace/switch; gated INSIDE the handler by the SHIPPED
#     `is_workspace_admin`, D7 — NO new check_arch allow-list line):
#       GET  /workspace/invites           (admin -> one-email-field form + CSRF cookie; non-admin/signed-out -> non-enumerable 404)
#       POST /workspace/invites           (email + _csrf -> insert_invite(created_by=admin) + InviteToken::new + emit link + best-effort email -> "invite sent" fragment; blank/bad email -> inline error; non-admin -> 404; CSRF-less -> refused)
#   * ACCEPTANCE (the SHIPPED PUBLIC route pair, EXTENDED with the member arm; the
#     invitee is signed out — has no account yet):
#       GET  /invites/accept?id&sig       (verify -> set-password form "join as a member" OR uniform refusal; NON-COMMITTAL; mints CSRF cookie)
#       POST /invites/accept              (id + sig + password + confirm + _csrf -> policy+confirm pre-consume -> DISPATCH member vs first-admin -> session -> 303 / OR inline error OR uniform refusal OR CSRF-refused)
#
# Driven adapters exercised (LAYER 3, @real-io) — NO MOCKS at the acceptance level:
#   * real Postgres via testcontainers PG16 + per-scenario schema — the `invites`
#     row (SHIPPED `used_at`/`used_by`, 0001_init.sql:99-100), `users`
#     (`email_lower UNIQUE`, 0001:19 — the OD-1 collision guard), `workspace_memberships`
#     (`role CHECK(admin|member)`, 0001:29 — the `member` role), `tower_sessions`;
#     ZERO migration (D8/adr-004);
#   * the real tower-sessions Postgres store (the auto-sign-in session);
#   * the real double-submit CSRF middleware (`csrf_middleware`) + `ensure_csrf_cookie`
#     minted on BOTH GETs (NFR-6 — both state-changing POSTs guarded);
#   * the SHIPPED `foundry_auth::InviteToken::new`/`verify` (HMAC over
#     invite_id||expires_at — issuance signs, accept re-verifies; the tamper oracle);
#   * the SHIPPED `foundry_auth::hash_password` (argon2id, on spawn_blocking) +
#     `check_password_policy` (min-6, NIST 800-63B, applied BEFORE any tx opens);
#   * the SHIPPED `Store::is_workspace_admin` (the issuance authz gate, GET + POST);
#   * the SHIPPED `Store::insert_invite` (created_by = the inviting admin — the
#     member/first-admin discriminator, D2/D3);
#   * the NEW `Store::create_member_and_consume` one-TX guarded-UPDATE-consume +
#     INSERT-user (UNIQUE-email collision -> ROLLBACK -> EmailCollision) + INSERT
#     `member` membership + set used_by (D4/D5/adr-002);
#   * the SHIPPED `Store::set_first_admin_password_and_consume` (the first-admin arm
#     of the kind dispatch — UNTOUCHED, regression-guarded);
#   * the SHIPPED `Store::resolve_active_workspace` (the landing tenant);
#   * the SHIPPED best-effort email seam (a send failure is non-fatal; the link is
#     still rendered — observed via the rendered fragment, NOT a live SMTP server).
# The Background seeds the issuing admin + workspace via the SHIPPED provisioning
# path, so the invite under test is minted by the REAL issuance handler — the
# issuance->accept handoff (the SAME invite_id + sig) is genuine, not synthesised.
#
# Refusal / non-enumerability decisions (the SECURITY CRUX, D5/D6/adr-002/adr-004):
#   * ISSUANCE (NFR-1): a non-admin OR signed-out GET/POST to `/workspace/invites`
#     returns the SHIPPED `resource_not_found_page()` — a 404 BYTE-IDENTICAL (status
#     + full body) to a never-existed path. No 401/403, no login redirect, no oracle
#     the surface exists (mirrors /admin/tokens + /admin/instance). Asserted against
#     a never-existed path as the control, for BOTH refusal causes, on GET + POST.
#   * ACCEPTANCE (NFR-3): every invalid accept reason — expired, already-used,
#     tampered-signature, unknown-id, AND email-already-a-user (the NEW arm, D5) —
#     returns ONE `invite_refusal_page()` that is BYTE-IDENTICAL (status 200 + full
#     body) across all five. The email collision is caught INSIDE the tx as the
#     `users.email_lower` UNIQUE violation and mapped to the SAME refusal — NEVER a
#     DB-constraint 500 (the HIGH-risk arm the DISCUSS flagged). Reason lives ONLY in
#     internal `tracing` keyed on `invite_id`; no `sig`/password in logs (NFR-5).
#   * A revert-reds-it litmus binds BOTH: collapsing any two refusal arms into
#     divergent responses MUST re-RED the byte-identity assertion (the slice-04
#     lesson: same-status hid 4 oracles — byte-identity is asserted on status AND
#     full body, never merely same-status).
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES — this file
# is Gherkin text, it adds NO undefined-symbol reference to any `.rs`, and it does
# NOT edit `acceptance.rs` (so `inventory` force-linking is untouched) -> NOT BROKEN.
# Cucumber-rs leaves unmatched step text as a runtime skip, not a compile error (the
# RED-state contract us-invite-accept.feature + us-mwt-web-provisioning.feature rely
# on). Genuine RED is MISSING_FUNCTIONALITY at runtime against the real
# testcontainers PG16:
#   1. The `member_invites.rs` web adapter — `show_invite_form` (GET) +
#      `submit_invite` (POST) — and the two `.route("/workspace/invites", get().post())`
#      lines on the SHARED layer of `build_router`, plus the member-invite form +
#      "invite sent" Askama templates, DO NOT EXIST YET — every issuance scenario
#      fails because the route is unknown / the handler is absent.
#   2. `Store::create_member_and_consume` DOES NOT EXIST YET — even once the accept
#      POST dispatches, the create-user + member-membership + consume + password
#      one-TX (with the UNIQUE-email collision arm) is missing (the genuinely-new
#      backend, D4/D5). No migration is owed (the columns shipped in 0001).
#   3. The member arm of the accept DISPATCH in `submit_accept`, and the EXTENSION of
#      `Store::invite_accept_view` to ALSO surface `invitee_email` + `created_by` for
#      the kind discriminator (D3/adr-003), DO NOT EXIST YET — a member invite cannot
#      be routed to the new tx yet.
# A scenario that reds for a REAL oracle (a divergent refusal arm, an email-collision
# 500, a status/body leak, a double-create, a CSRF bypass, an invite consumed on a
# rejected password, a member reaching issuance, or a first-admin regression) is
# flagged in distill/upstream-issues.md, not silently accepted.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter scenarios
# are EXAMPLE-BASED (NOT property-based); every sad / evil-user / adversarial path is
# enumerated explicitly; NO PBT machinery at this layer. The `@property`-tagged
# scenarios (issuance + accept non-enumerability, single-use + single-create under
# concurrency, no-secret-leakage) remain EXAMPLE-PINNED at layer 3 (matching the
# invite-accept + slice-04 + web-provisioning convention), with their
# universal-invariant SHAPE preserved in the title for the DELIVER crafter. Mandate 8
# state-delta is a layers-1-3 Python pilot; NO `state_delta.rs` Rust port exists
# (matching every shipped foundry-acceptance feature), so LAYER-3 assertions are
# traditional assertions over port-exposed web observables: rendered page/fragment
# substrings (workspace name, set-password form, "invite sent" link, inline error),
# HTTP refusal status + BYTE-IDENTICAL refusal body, redirect-and-landed tenant,
# post-consume `invites.used_at` set exactly once, post-error invite still live,
# exactly-one user + one membership, and a log scan free of `sig`/password. See
# docs/architecture/atdd-infrastructure-policy.md for the per-port mechanism.
#
# Scope (v1, RATIFIED): MEMBER role only. The member-invite ISSUANCE web form
# (US-01) + the account-CREATING member ACCEPT (US-02) + the non-enumerable +
# single-use + CSRF + no-leak security gate (US-03) + the inline recovery (US-04).
# Inviting as ADMIN, bulk invites, invite revocation/resend, a CLI-native
# `foundry workspace invite` command, and multi-workspace-membership-via-invite for
# an email that is already a user (OD-1: refused non-enumerably in v1) are explicitly
# OUT — do not add them here.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time
# DELIVER cycle; DELIVER unskips one scenario per RED->GREEN->COMMIT cycle; @pending
# is excluded by the harness default + @all lanes per acceptance.rs).

@workspace-member-invites @real-io @driving_adapter
Feature: A workspace admin invites a teammate, who joins by creating an account from the link
  Dana Reyes is the admin of the "Northwind" workspace. Her teammate Sam Okafor
  needs in, but today only the instance super-admin can mint an invite — Dana has to
  file a ticket and wait. With this feature Dana opens the workspace member-invite
  form, types Sam's email, clicks "Send invite", and sees "Invite sent — share this
  link", valid 7 days. Sam has never used Foundry — he has no account at all. He
  opens the link, sets his own password, submits once, and lands straight on the
  Northwind dashboard signed in — a new account created for him, joined as a member,
  in one atomic step, seeing only Northwind. A plain member or a signed-out probe of
  the issuance form gets a generic 404 that does not even admit the surface exists.
  Every bad accept link — expired, used, tampered, unknown — AND an email that
  already has an account show ONE calm, byte-identical "invite is no longer valid"
  page that leaks nothing, never a server error. An invite creates exactly one
  account, even under a double-click race. Both state-changing submits are
  forgery-protected. A password typo or a blank email is fixed inline without burning
  the invite. And the shipped first-admin claim flow still works. Proven with REAL
  invites minted by the real issuance handler over the real session + CSRF + Postgres
  machinery — no mocks.

  Background:
    Given Dana Reyes is signed in as an admin of the "Northwind" workspace

  # ----------------------------------------------------------------------------
  # 1. WALKING SKELETON — the demo-able headline value, end-to-end through BOTH new
  #    driving adapters chained: Dana issues a member invite, then Sam (no account)
  #    accepts it, has an account created + joins as a member + is signed in, and
  #    lands on Northwind seeing only that tenant. The thinnest cut that proves the
  #    NEW issuance route + the NEW `create_member_and_consume` tx + the member arm
  #    of the accept dispatch wire end-to-end through session + CSRF + Postgres.
  #    (US-01 + US-02: AC-01.2/01.3 + AC-02.3/02.4/02.5.)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-01 @us-02
  Scenario: An admin invites a teammate who creates an account and joins as a member
    When Dana invites "sam.okafor@northwind.example" to "Northwind"
    And Sam opens his invite link and sets a password meeting the strength policy
    Then a new account is created for "sam.okafor@northwind.example"
    And Sam is signed in on the "Northwind" workspace without a separate login step
    And Sam is a member of "Northwind" and sees no data from any other workspace
    And his invite is recorded as used exactly once

  # ----------------------------------------------------------------------------
  # ISSUANCE (US-01) — the admin-facing member-invite surface.
  # ----------------------------------------------------------------------------

  # 2. The issuance form renders for a signed-in workspace admin, naming the
  #    workspace (AC-01.1). The arrival step of the issuance chain — its Given+When
  #    is reused by scenarios 3 and 4 (Pillar 2 chained narrative).
  @us-01
  Scenario: An admin opens the member-invite form for her workspace
    When Dana opens the member-invite form
    Then she sees a one-email-field form to invite a member to "Northwind"

  # 3. POST with a valid email creates the invite and shows the shareable accept link
  #    valid 7 days (AC-01.2/01.3/01.5). Its Given chains off scenario 2's form.
  @us-01
  Scenario: Submitting a valid email creates an invite and shows a shareable link
    Given Dana has opened the member-invite form for "Northwind"
    When Dana submits "sam.okafor@northwind.example"
    Then an invite to "Northwind" is created for "sam.okafor@northwind.example"
    And Dana sees a confirmation with a shareable accept link valid for 7 days
    And the emitted signature verifies against that invite

  # 4. The link is shown even when the invite email fails to send — best-effort email
  #    is non-fatal; Dana can paste the link manually (AC-01.4, I-E5).
  @us-01
  Scenario: The shareable link is shown even when the invite email fails to send
    Given the mail service is unavailable for "Northwind"
    When Dana submits "sam.okafor@northwind.example" on the member-invite form
    Then the invite is still created
    And Dana still sees the shareable accept link to paste manually

  # 5. Two invites to the same email are independent live invites, each with its own
  #    link (US-01 domain example 3 — each invite is single-use and independent).
  @us-01
  Scenario: An admin can issue a second independent invite to the same email
    Given Dana already issued "sam.okafor@northwind.example" an invite yesterday that was never used
    When Dana issues another invite to "sam.okafor@northwind.example"
    Then a second independent live invite is created with its own link

  # ----------------------------------------------------------------------------
  # ACCEPTANCE (US-02) — the account-CREATING member accept.
  # ----------------------------------------------------------------------------

  # 6. The GET accept page renders the set-password form for a live member invite,
  #    naming the workspace and "join as a member" (AC-02.1). The arrival step of the
  #    accept chain — its Given+When is reused by scenarios 7, 17, 18, 19, 20, 21.
  @us-02
  Scenario: A live member invite renders a set-password form naming the workspace
    Given Dana issued Sam a live member invite for "Northwind" two hours ago
    And Sam has no Foundry account yet
    When Sam opens his invite link
    Then he sees a set-password form to join "Northwind" as a member

  # 7. The GET is NON-COMMITTAL — opening the page creates no account and consumes
  #    nothing (AC-02.2). The TOCTOU-safety foundation: only the POST tx is
  #    authoritative. Its Given reuses scenario 6's Given+When (chained narrative).
  @us-02
  Scenario: Opening the member-accept page creates no account and consumes nothing
    Given Sam has opened his live member invite for "Northwind" and seen the set-password form
    Then no account exists yet for "sam.okafor@northwind.example"
    And his invite is still live and unconsumed

  # 8. A near-fresh invite (issued 20 seconds ago) accepts immediately — creates the
  #    member account and signs in (US-02 domain example 2).
  @us-02
  Scenario: A near-fresh member invite accepts immediately
    Given Dana issued Priya Shah a member invite for "Northwind" twenty seconds ago
    When Priya opens her link and sets a valid password
    Then a new member account is created for Priya and she is signed in on "Northwind"

  # 9. Boundary — an invite opened just INSIDE its expiry window (issued 6 days 23
  #    hours ago, i.e. expires_at - 1s) still renders and accepts (AC-02.7). Pairs
  #    with scenario 13 (just outside).
  @us-02
  Scenario: A member invite opened just inside its expiry window is accepted
    Given Sam's member invite is one second away from expiring and has not been used
    When Sam opens his link and sets a valid password
    Then his member account is created and he is signed in on "Northwind"

  # 10. The new member lands on invites.workspace_id and sees ONLY that tenant's data
  #     (AC-02.4 — tenant landing). The isolation half of the join.
  @us-02
  Scenario: The new member lands on the inviting workspace and sees only its data
    Given Sam has accepted his member invite and is signed in on "Northwind"
    When Sam views his workspace
    Then he sees only "Northwind" data
    And he sees no data from any other workspace

  # 11. The new member has the MEMBER role, not admin — he 404s on the issuance
  #     surface (AC-02.6, BR-2 — privilege scope). This is also the regression guard
  #     that the issuance surface stays admin-gated against a freshly-joined member.
  @us-02
  Scenario: A newly joined member cannot reach the admin issuance surface
    Given Sam has accepted his member invite and is signed in on "Northwind"
    When Sam opens the member-invite form
    Then he sees a generic "not found"
    And nothing reveals that the issuance surface exists

  # 12. FIRST-ADMIN REGRESSION GUARD — a first-admin invite (the consumer IS
  #     created_by, the account pre-exists) still routes to the SHIPPED tx and signs
  #     in unchanged; the data-derived kind dispatch (D3) did NOT break the shipped
  #     flow. Proves NO second account is created for the first-admin arm.
  @us-02 @verify-path-unchanged
  Scenario: A first-admin invite still routes to the shipped accept path
    Given a super-admin provisioned the "Globex" workspace and seeded Priya Nair as its first-admin with a live invite
    When Priya opens her first-admin invite link and sets a valid password
    Then Priya is signed in on the "Globex" workspace without a separate login step
    And no second account is created for Priya
    And her first-admin invite is recorded as used exactly once

  # ----------------------------------------------------------------------------
  # SECURITY GATE (US-03) — non-enumerable refusals + single-use + CSRF + no-leak.
  # ----------------------------------------------------------------------------

  # 13. Boundary — an invite opened just OUTSIDE its expiry window (expires_at + 1s)
  #     is refused with the uniform page (AC-03.4). Expiry enforced on GET liveness.
  #     Pairs with scenario 9.
  @us-03 @error
  Scenario: A member invite opened just past its expiry window is refused
    Given Sam's member invite expired one second ago
    When Sam opens his invite link
    Then he sees the standard "invite is no longer valid" page

  # 14. The canonical accept refusal arm — an EXPIRED member invite is refused without
  #     leaking existence and advises asking the admin to re-issue (AC-03.2/03.3,
  #     A-E1). Scenarios 15/16/17 assert byte-identity AGAINST this one.
  @us-03 @error
  Scenario: An expired member invite is refused without leaking existence
    Given Sam's member invite expired one day ago
    When Sam opens his invite link
    Then he sees the standard "invite is no longer valid" page
    And the page reveals nothing about whether any account or workspace exists
    And the page advises asking the workspace administrator to re-issue the invite

  # 15. A TAMPERED signature is refused IDENTICALLY to an expired link — the HMAC
  #     tamper oracle fails before any DB hit; byte-identical response (A-E3, AC-03.2).
  @us-03 @error
  Scenario: A tampered signature is refused identically to an expired link
    Given Sam's member invite is live but the signature in the link has been altered by one character
    When Sam opens the tampered link
    Then he sees the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal

  # 16. An UNKNOWN invite id is refused IDENTICALLY — a prober cannot tell the id
  #     never existed (A-E4, AC-03.2).
  @us-03 @error
  Scenario: An unknown invite id is refused identically to every other reason
    Given an invite id that was never issued
    When someone opens an accept link with that id
    Then they see the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal
    And nothing reveals whether that id, account, or workspace exists

  # 17. EMAIL-ALREADY-A-USER — the NEW collision arm (D5, OD-1, A-E9, the HIGH-risk
  #     arm). The invitee's email already maps to an existing Foundry user; the
  #     create-user step aborts the tx on the UNIQUE violation; the response is
  #     byte-identical to the expired-link refusal — NEVER a 500 — no second account,
  #     invite NOT consumed (AC-03.8). This is the genuinely-new refusal branch.
  @us-03 @error
  Scenario: A member invite whose email already has an account is refused without leaking that fact
    Given Dana issued a member invite for an email that already has a Foundry account
    When that invitee opens the link and submits a valid password
    Then they see the standard "invite is no longer valid" page
    And the response is byte-identical to the expired-invite refusal
    And no second account is created and the invite is not consumed

  # 18. @property — ACCEPT NON-ENUMERABILITY: the five invalid reasons {expired,
  #     already-used, tampered-signature, unknown-id, email-already-a-user} ALL
  #     produce a byte-identical user-visible refusal (status + full body); they
  #     differ ONLY in internal logging. The revert-reds-it litmus binds it.
  #     Example-pinned at LAYER 3 (Mandate 11). (AC-03.2/03.3/03.8, NFR-3 — the crux.)
  @us-03 @error @property
  Scenario: Accept refusals are byte-identical across all five invalid reasons
    Given an expired invite, an already-used invite, a tampered-signature link, an unknown-id link, and an email-already-a-user invite
    When each accept is attempted
    Then all five produce a byte-identical user-visible refusal page
    And the email-collision refusal is never a server error
    And they differ only in internal logging, never in the observable response

  # 19. SINGLE-USE — a consumed member invite re-opened is refused; no second account
  #     and no session (AC-03.5, A-E2). Its Given reuses the walking skeleton's
  #     successful accept (chained narrative).
  @us-03 @error
  Scenario: A consumed member invite can never be used again
    Given Sam has already created his account and joined "Northwind" via his invite link
    When Sam opens the same invite link again
    Then he sees the standard "invite is no longer valid" page
    And no second account is created and no session is created

  # 20. @property — SINGLE-USE + SINGLE-CREATE UNDER CONCURRENCY: two accept
  #     submissions for one live member invite race; the guarded-UPDATE means exactly
  #     one creates the account + joins + signs in, the other gets the uniform
  #     refusal, and exactly one user + one membership + one consumed invite exist.
  #     The race oracle for NFR-2. Example-pinned at LAYER 3 (Mandate 11). (AC-03.6, A-E7.)
  @us-03 @error @property
  Scenario: Concurrent accepts of one member invite create the account exactly once
    Given Sam's member invite is live
    When two accept submissions for the same invite arrive concurrently
    Then exactly one submission creates the account, joins, and signs Sam in
    And the other receives the standard "invite is no longer valid" page
    And exactly one user and one membership are created and the invite is used exactly once

  # 21. TOCTOU — a link consumed in the GET->POST window is refused by the consume tx
  #     guard; expiry is enforced INSIDE the tx, not just on GET (AC-03.7). No second
  #     account is created (the guard returns 0 rows and rolls back).
  @us-03 @error
  Scenario: A member invite consumed between opening the page and submitting is refused by the transaction guard
    Given Sam has opened his live member invite for "Northwind" and seen the set-password form
    And the same invite is consumed by another submission before Sam submits
    When Sam submits a valid password on his now-stale page
    Then he sees the standard "invite is no longer valid" page
    And no account is created and the invite stays used exactly once

  # 22. ISSUANCE NON-ENUMERABILITY — a signed-in non-admin (a plain member) GET/POST
  #     to the issuance surface is refused byte-identically to a never-existed path;
  #     no invite created (AC-03.1, I-E1). The control is a path that never existed.
  @us-03 @error
  Scenario: A non-admin cannot tell the issuance surface exists
    Given Marco is signed in as a plain member of "Northwind"
    When Marco opens the member-invite page and submits an email
    And Marco requests a path that never existed
    Then each member-invite response is byte-identical to the never-existed path
    And no invite is created
    And nothing reveals that the issuance surface exists

  # 23. ISSUANCE NON-ENUMERABILITY — a SIGNED-OUT GET/POST to the issuance surface is
  #     refused byte-identically to the non-admin refusal AND to a never-existed path;
  #     no "sign in to invite" oracle (AC-03.1, I-E2).
  @us-03 @error @property
  Scenario: A signed-out caller cannot tell the issuance surface exists
    Given no one is signed in
    When a signed-out caller opens the member-invite page and a never-existed path
    Then each member-invite response is byte-identical to the never-existed path
    And the signed-out refusal is byte-identical to the non-admin refusal
    And no invite is created

  # 24. CSRF — BOTH state-changing POSTs without a valid double-submit token are
  #     refused by the SHIPPED csrf_middleware BEFORE the handler runs: the issuance
  #     POST creates no invite; the accept POST creates no account and consumes
  #     nothing (AC-03.9, NFR-6, I-E4 + A-E8).
  @us-03 @error
  Scenario: Both submissions are refused without a valid security token
    Given a forged issuance submission and a forged accept submission for a live invite, each without a valid security token
    When each reaches its surface
    Then each is refused by the request-forgery protection
    And no invite is created, no invite is consumed, and no account is created

  # 25. @property — NO-SECRET-LEAKAGE: across a full issue + accept + refusal cycle,
  #     the application logs contain neither the invite `sig` value nor any submitted
  #     password (the refusal reason lives in tracing keyed on invite_id only).
  #     Example-pinned at LAYER 3 (Mandate 11). (AC-03.10, NFR-5.)
  @us-03 @error @property
  Scenario: No invite signature or password ever appears in the logs
    Given Dana issues an invite, Sam completes a successful accept, and a hostile prober is refused
    When the application logs for the full cycle are examined
    Then no invite signature value appears in the logs
    And no submitted password appears in the logs

  # ----------------------------------------------------------------------------
  # INLINE RECOVERY (US-04) — correct mistakes without losing the invite.
  # ----------------------------------------------------------------------------

  # 26. A WEAK password (below min-6) is corrected inline; the policy check runs
  #     BEFORE the consume tx opens, so the invite is NOT consumed and NO account is
  #     created; no session (AC-04.1, A-E5).
  @us-04 @error
  Scenario: A weak password is corrected inline and creates no account
    Given Sam has opened his live member invite for "Northwind" and seen the set-password form
    When he submits a password below the strength policy
    Then he sees an inline error explaining the minimum password length
    And his invite is still live and unconsumed
    And no account is created and no session is created

  # 27. A MISMATCHED confirmation is corrected inline; the invite is NOT consumed and
  #     no account is created (AC-04.2, A-E6).
  @us-04 @error
  Scenario: A mismatched confirmation is corrected inline and creates no account
    Given Priya Shah has opened her live member invite for "Northwind" and seen the set-password form
    When her confirmation does not match her new password
    Then she sees an inline error that the passwords do not match
    And her invite is still live and unconsumed and no account is created

  # 28. A BLANK email on the issuance form is corrected inline; NO invite is created
  #     (AC-04.3, FR-3, I-E3).
  @us-04 @error
  Scenario: A blank email on the issuance form is corrected inline
    Given Dana has opened the member-invite form for "Northwind"
    When Dana submits the form with an empty email
    Then she sees an inline error asking for an email address
    And no invite is created

  # 29. RE-ATTEMPT — after an inline password error, re-submitting a valid password on
  #     the SAME live invite completes the join (AC-04.5, the recoverability proof).
  #     Its Given chains off scenario 26's left-live invite.
  @us-04
  Scenario: A valid retry on the same member invite after an error completes the join
    Given Sam was shown an inline password error and his member invite is still live
    When he submits a valid password on the same invite and confirms it
    Then his member account is created and he is signed in on "Northwind"
    And his invite is recorded as used exactly once

  # 30. BOUNDARY — a password EXACTLY at the minimum length (6 characters) is
  #     accepted and creates the member account (AC-04.4, NFR-4 — "at least 6").
  @us-04
  Scenario: A password exactly at the minimum length is accepted
    Given Sam has opened his live member invite for "Northwind" and seen the set-password form
    When he submits a six-character password and confirms it
    Then his member account is created and he is signed in on "Northwind"
