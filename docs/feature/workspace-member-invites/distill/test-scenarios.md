# DISTILL — Test Scenario Catalog: workspace-member-invites

> Quinn (nw-acceptance-designer), DISTILL wave. Lang: Rust. Framework: cucumber-rs.
> Test type: core feature. Integration: real services — testcontainers PG16 + the in-process
> axum router; LAYER 3 `@real-io`; NO mocks. Single SSOT for executable scenarios:
> `crates/foundry-acceptance/tests/features/us-member-invites.feature` (30 scenarios).
> Mirrors the shipped `us-invite-accept.feature` (first-admin accept) + `us-mwt-web-provisioning.feature`
> (admin-web issuance + non-enumerable gate) house style.

## Reconciliation HARD GATE — PASSED

Read `discuss/{user-stories,acceptance-criteria,requirements}.md` + `design/{wave-decisions,architecture}.md`.
DEVOPS directory absent for this feature → WARN, defaults applied (real PG16 testcontainers +
in-process axum router, per the established `foundry-acceptance` convention + the Project
Infrastructure Policy). **Zero contradictions** across DISCUSS / DESIGN / (DEVOPS-default): DESIGN
ratifies every DISCUSS open decision (notably OD-1 email-collision → non-enumerable refusal) and adds
no contradicting choice. Reconciliation passed — proceed.

## Tier decision (Mandate 10)

**Tier A only.** Every scenario runs at LAYER 3 (real adapter: testcontainers PG16 + the in-process
axum router over real HTTP) — the production composition root via `foundry_app::test_support::spawn_app`.
Tier B (state-machine PBT, in-memory doubles) is NOT added: there is no Rust `state_delta.rs` /
`InMemoryComposition` port in this project, and the journey's observable contract is web-surface +
DB-row state proven through real I/O — the in-memory doubles would not honor the `users.email_lower`
UNIQUE / `workspace_memberships.role` CHECK / guarded-UPDATE concurrency semantics that ARE the
feature's risk. Per Mandate 11 every sad path is example-based; per Mandate 9 no PBT machinery at
LAYER 3; `@property` scenarios stay example-pinned with their invariant SHAPE in the title.

## Scenario catalog (30) — story / NFR / decision traceability + RED-state contract

Legend: WS = `@walking_skeleton @wiring_e2e` (runs in default + `@all` lanes); rest `@pending`
(excluded everywhere until DELIVER unskips one per RED→GREEN→COMMIT cycle).

| # | Scenario (title) | Tags | Stories | AC | NFR/FR | Decisions | RED reason (what's MISSING) |
|---|---|---|---|---|---|---|---|
| 1 | An admin invites a teammate who creates an account and joins as a member | WS @us-01 @us-02 | US-01,US-02 | AC-01.2/01.3, AC-02.3/02.4/02.5 | FR-2,FR-5,FR-6,NFR-2 | D1,D2,D3,D4 | `member_invites.rs` issuance handlers + routes; `create_member_and_consume` tx; member arm of accept dispatch — all absent |
| 2 | An admin opens the member-invite form for her workspace | @us-01 | US-01 | AC-01.1 | FR-1,NFR-1 | D1,D7 | `show_invite_form` GET handler + form template absent |
| 3 | Submitting a valid email creates an invite and shows a shareable link | @us-01 | US-01 | AC-01.2/01.3/01.5 | FR-2 | D2 | `submit_invite` POST handler + "invite sent" fragment absent |
| 4 | The shareable link is shown even when the invite email fails to send | @us-01 | US-01 | AC-01.4 | FR-2 | D2 | best-effort-email-non-fatal path in `submit_invite` absent |
| 5 | An admin can issue a second independent invite to the same email | @us-01 | US-01 | (US-01 ex.3) | FR-2,BR-3 | D2 | `submit_invite` (independent invite rows) absent |
| 6 | A live member invite renders a set-password form naming the workspace | @us-02 | US-02 | AC-02.1 | FR-4 | D3 | accept GET "join as a member" copy + `invite_accept_view` extension absent |
| 7 | Opening the member-accept page creates no account and consumes nothing | @us-02 | US-02 | AC-02.2 | FR-4 | D3 | (non-committal GET) member dispatch absent |
| 8 | A near-fresh member invite accepts immediately | @us-02 | US-02 | (US-02 ex.2) | FR-5,NFR-2 | D4 | `create_member_and_consume` absent |
| 9 | A member invite opened just inside its expiry window is accepted | @us-02 | US-02 | AC-02.7 | NFR-2 | D4 | `create_member_and_consume` + in-tx expiry guard absent |
| 10 | The new member lands on the inviting workspace and sees only its data | @us-02 | US-02 | AC-02.4 | FR-6 | D4 | member arm + `resolve_active_workspace` landing for new user absent |
| 11 | A newly joined member cannot reach the admin issuance surface | @us-02 | US-02 | AC-02.6 | FR-6,BR-2,NFR-1 | D1,D7 | issuance admin-gate + `member`-role membership from new tx absent |
| 12 | A first-admin invite still routes to the shipped accept path | @us-02 @verify-path-unchanged | US-02 | (regression guard) | — | D3 | the kind DISPATCH that routes first-admin → SHIPPED tx absent (guards no regression) |
| 13 | A member invite opened just past its expiry window is refused | @us-03 @error | US-03 | AC-03.4 | NFR-2 | D6 | accept GET liveness + uniform refusal for member invite absent |
| 14 | An expired member invite is refused without leaking existence (CANONICAL arm) | @us-03 @error | US-03 | AC-03.2/03.3 | NFR-3,FR-7 | D6 | uniform `invite_refusal_page()` wired to member accept absent |
| 15 | A tampered signature is refused identically to an expired link | @us-03 @error | US-03 | AC-03.2 | NFR-3 | D6 | byte-identical refusal arm absent |
| 16 | An unknown invite id is refused identically to every other reason | @us-03 @error | US-03 | AC-03.2 | NFR-3 | D6 | byte-identical refusal arm absent |
| 17 | A member invite whose email already has an account is refused (the NEW collision arm) | @us-03 @error | US-03 | AC-03.8 | NFR-3,BR-5 | D5,OD-1 | UNIQUE-violation-catch → `EmailCollision` → uniform refusal (NOT a 500) absent |
| 18 | Accept refusals are byte-identical across all five invalid reasons | @us-03 @error @property | US-03 | AC-03.2/03.3/03.8 | NFR-3 | D5,D6 | the five-arm uniform refusal (incl. collision) absent |
| 19 | A consumed member invite can never be used again | @us-03 @error | US-03 | AC-03.5 | NFR-2 | D4 | single-use guard on member tx absent |
| 20 | Concurrent accepts of one member invite create the account exactly once | @us-03 @error @property | US-03 | AC-03.6 | NFR-2 | D4 | guarded-UPDATE-consume race-safety in `create_member_and_consume` absent |
| 21 | A member invite consumed between opening the page and submitting is refused by the transaction guard | @us-03 @error | US-03 | AC-03.7 | NFR-2 | D4,D6 | in-tx 0-rows guard (TOCTOU) absent |
| 22 | A non-admin cannot tell the issuance surface exists | @us-03 @error | US-03 | AC-03.1 | NFR-1 | D1,D7 | issuance `is_workspace_admin` gate + `resource_not_found_page()` posture absent |
| 23 | A signed-out caller cannot tell the issuance surface exists | @us-03 @error @property | US-03 | AC-03.1 | NFR-1 | D1,D7 | issuance gate (signed-out arm, byte-identical to non-admin) absent |
| 24 | Both submissions are refused without a valid security token | @us-03 @error | US-03 | AC-03.9 | NFR-6 | D1 | CSRF screening on both new POST surfaces absent |
| 25 | No invite signature or password ever appears in the logs | @us-03 @error @property | US-03 | AC-03.10 | NFR-5 | D1 | tracing-keyed-on-invite_id discipline on new handlers absent |
| 26 | A weak password is corrected inline and creates no account | @us-04 @error | US-04 | AC-04.1 | FR-8,NFR-4 | D3 | pre-consume `check_password_policy` on member arm absent |
| 27 | A mismatched confirmation is corrected inline and creates no account | @us-04 @error | US-04 | AC-04.2 | FR-8 | D3 | pre-consume confirm-match on member arm absent |
| 28 | A blank email on the issuance form is corrected inline | @us-04 @error | US-04 | AC-04.3 | FR-3 | D2 | issuance inline email validation absent |
| 29 | A valid retry on the same member invite after an error completes the join | @us-04 | US-04 | AC-04.5 | FR-8 | D3 | (recoverability) member arm + left-live invite absent |
| 30 | A password exactly at the minimum length is accepted | @us-04 | US-04 | AC-04.4 | NFR-4 | D3 | member arm accepting 12-char password absent |

## Story coverage (every US exercised)

- **US-01 Issue (admin-gated)**: scenarios 1,2,3,4,5 + the issuance halves of 22,23,24,25,28. ✓
- **US-02 Accept (account-creating)**: scenarios 1,6,7,8,9,10,11,12. ✓
- **US-03 Safe & honest (non-enumerable + single-use + CSRF + no-leak)**: scenarios 13–25. ✓
- **US-04 Inline recovery**: scenarios 26,27,28,29,30. ✓

## NFR coverage (all 6 security NFRs + the 8 FRs)

| NFR | Scenarios |
|---|---|
| NFR-1 issuance authz non-enumerable | 11, 22, 23 |
| NFR-2 single-use atomic accept (race/TOCTOU) | 1, 8, 9, 19, 20, 21 |
| NFR-3 non-enumerable accept refusals (incl. collision, not a 500) | 13, 14, 15, 16, 17, 18 |
| NFR-4 password strength min-12 | 26, 30 |
| NFR-5 no secret leakage | 25 |
| NFR-6 CSRF on both POSTs | 24 |
| FR-1..8 | 2 (FR-1), 3/4/5 (FR-2), 28 (FR-3), 6/7 (FR-4), 1/8/9 (FR-5), 10/11 (FR-6), 14 (FR-7), 26/27/29 (FR-8) |

NFR-7 (accessibility) is non-gating per DISCUSS/DESIGN — deferred to implementation review, no scenario.

## Decision coverage (D1–D8 + OD-1)

D1 (issuance NEW + accept EXTENDED driving adapters): 1,2,3,22,24. · D2 (reuse `insert_invite`): 3,4,5,28.
· D3 (data-derived kind dispatch, no column): 6,7,12,26,27. · D4 (`create_member_and_consume` one-TX):
1,8,9,10,19,20,21. · D5/OD-1 (collision → uniform refusal, not 500): 17,18. · D6 (refusal posture
reused, byte-identical): 13,14,15,16,18. · D7 (LAYER-1e no allow-list line): 11,22,23. · D8 (no
migration): structural — every scenario runs against the shipped 0001 schema, no migration staged.

## Adapter coverage (Mandate 6 — every driven adapter has a @real-io scenario)

| Driven adapter | @real-io scenario | Covered by |
|---|---|---|
| `Store::insert_invite` (issuance row) | YES | 1, 3, 5 |
| `Store::is_workspace_admin` (issuance gate) | YES | 22, 23 (refuse), 2 (admit) |
| `Store::create_member_and_consume` (NEW one-TX) | YES | 1, 8, 9, 17 (collision), 20 (race), 21 (TOCTOU) |
| `Store::set_first_admin_password_and_consume` (SHIPPED, first-admin arm) | YES | 12 (regression guard) |
| `Store::invite_accept_view` (EXTENDED: invitee_email + created_by) | YES | 6, 12 (dispatch read) |
| `Store::resolve_active_workspace` (landing) | YES | 1, 10 |
| `InviteToken::new`/`verify` (HMAC) | YES | 3 (sign verifies), 15 (tamper) |
| `hash_password` / `check_password_policy` (argon2id, min-12) | YES | 1, 26, 30 |
| `csrf_middleware` (both POSTs) | YES | 24 |
| best-effort email seam | YES | 4 |
| tower-sessions store (auto-sign-in) | YES | 1, 8, 19 (no session on refusal) |

Zero "NO — MISSING" rows. All driven adapters exercised through real I/O.

## Pre-DELIVER fail-for-the-right-reason gate (deferred to DELIVER PREPARE/RED)

`cargo` was NOT run this session (per task instruction). The crate COMPILES by the RED-state contract
(Gherkin-only addition; `acceptance.rs` untouched; cucumber-rs treats unmatched steps as runtime skip,
not a compile error). DELIVER's RED phase confirms each unskipped scenario fails as
`MISSING_FUNCTIONALITY` (route unknown / handler absent / `create_member_and_consume` absent / member
dispatch arm absent) against real testcontainers PG16 — never IMPORT_ERROR / FIXTURE_BROKEN. A scenario
that reds for a REAL oracle (divergent refusal arm, collision-500, double-create, CSRF bypass,
consumed-on-rejected-password, member-reaches-issuance, first-admin regression) is flagged in
`upstream-issues.md`, not silently accepted.
