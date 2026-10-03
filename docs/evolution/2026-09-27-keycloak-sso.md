# keycloak-sso — evolution archive (increment D3a: role-gated opt-in provisioning)

**What this archive covers.** The base feature shipped in `c755003` (released in
v0.4.0) with no evolution doc: federated sign-in that links a Keycloak identity to a
foundry account that already exists and provisions nothing. This archive covers only
the **D3a increment** from 2026-09-27. With D3a, a Keycloak identity with no foundry
account can get one, if all of the following hold:
- the operator names a Keycloak realm role in `FOUNDRY_OIDC_PROVISION_ROLE`;
- the identity's ID token carries that role;
- the provider has confirmed the address;
- the instance has been claimed.

The new account is a password-less `member`.

Waves for the increment: DISCUSS addendum (D3a, US-06) → DESIGN addendum (DDD-13..22) →
DISTILL increment → DELIVER. No DEVOPS wave ran (DISTILL: WARN, default matrix). The
increment was delivered in **no-commit mode**, with all 4 COMMIT phases logged
`APPROVED_SKIP`. The orchestrator commits it after the closing gate.

Final `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: **RED, 854/855 scenarios (5971/5972 steps), 2026-09-27; the one failure is not D3a.** `us-03-backup-restore` "`foundry doctor backup-verify` ... reports row counts" printed an empty `row-counts:` block. `count_rows` (`admin_cli.rs:1634`) shells out to a bare `psql`, but this host has no Postgres client. `439ee6f` (2026-09-04) routed `pg_restore` through a container shim (`FOUNDRY_PG_RESTORE`) but not `psql`, and `count_rows` swallows every error as "table not present". It failed 3/3 in isolation with `FOUNDRY_ACCEPTANCE_TAGS=us-03-cli`. D3a touches neither `admin_cli.rs` nor the backup steps. The user chose to commit D3a and fix the `psql` seam as a separate bugfix.

> **Addendum 2026-09-28:** the one gate failure above, the host-`psql` gap in `backup-verify`, is fixed by `fix-backup-verify-fail-open`, which counts rows in the binary and fails closed. The full gate then went 855/855 green.

> **Addendum 2026-10-03:** a later increment closed OD-11, un-pending the 23 base
> `keycloak-sso.feature` scenarios against the shipped flow with no production change.
> See § "2026-10-03 increment: OD-11" below.

## Business context

Before D3a, every operator had to be invited or created by hand before their cluster
identity could reach the board. The job is `job-sso-signin`, and the persona is the same
cluster operator as the base feature. **D3a supersedes only the "provision nothing"
half of D3.** It is opt-in and role-gated:
- With the variable unset or blank, foundry behaves exactly as `c755003` did.
- An existing account is still only linked.
- The local password path, the bootstrap claim and invite-accept are unchanged.

## What shipped

- **Migration `0016_nullable_password_hash.sql`.** It runs `ALTER TABLE users ALTER
  COLUMN password_hash DROP NOT NULL`, with no backfill and no default, and leaves
  `email_lower UNIQUE` untouched (DDD-13). **It is one-way** once a NULL-hash row
  exists. A revert is a forward migration: first delete those accounts or give them
  hashes. It is never a down migration.
- **`UserRow.password_hash: Option<String>`**, handled explicitly by every reader, with
  no sentinel string. `signin::verify_against(password, Option<&str>)` is now the one
  verifier behind both the password door and change-password reauth. With no hash, it
  still verifies against `known_bad_hash()` and answers `false`, never returning early
  (DDD-19). So a password-less account is refused with the unknown-address status, body
  and wall-clock. Change-password now reads the hash through `find_user_by_id`.
- **`foundry-oidc` extracts, never decides (DDD-14).**
  - `IdTokenClaims` gains `realm_access.roles`, `name` and `preferred_username`, all
    `serde(default)`, so older tokens validate as before.
  - `IdentityClaims::has_realm_role` is an exact, case-sensitive match on realm roles
    only; `resource_access` client roles never count (OD-9).
  - `OidcConfig::with_provision_role` treats blank or whitespace as `None`, and
    `OidcProvider::provision_role()` exposes the result read-only.
  - The crate still depends on neither `foundry-store` nor `foundry-auth`.
- **Find-or-provision in `oidc::callback` (DDD-15).** After the unchanged exchange and
  `email_verified` checks, the steps are:
  1. An existing account is linked, and the role is ignored.
  2. If no role is configured, the newcomer is refused `no foundry account` (exactly D3).
  3. If the identity lacks the role, it is refused `identity lacks provision role`.
  4. If the instance is unclaimed, the newcomer is refused `no workspace to provision into`.
  5. Otherwise the account is provisioned, and the session is established with the
     shipped `signin::establish_session`.

  Every refusal is the shipped generic refusal (D7/DDD-11). A successful provision logs
  one `info` line, `oidc identity provisioned`, with ids only (DDD-20).
- **`Store::provision_federated_member` (DDD-16/17).** One transaction does all of this:
  - it picks the instance's ORIGINAL workspace, `ORDER BY created_at, id LIMIT 1`;
  - it inserts a user with `password_hash = NULL` under `ON CONFLICT (email_lower) DO
    NOTHING`, then re-reads;
  - it adds one `member` membership.

  It returns `Created`, `Existing` (the loser of a concurrent race, untouched) or
  `NoWorkspace`. It never grants `admin`.
- **Greeting name (DDD-18).** The name is the first of `name` → `preferred_username` →
  the email local-part that is non-blank once trimmed and at most 64 characters. A
  longer candidate is skipped, never truncated (OD-7). If no candidate qualifies, the
  identity is refused. DDD-18 calls that case unreachable, because RFC 5321 caps a
  local-part at 64 octets.
- **Reset for password-less accounts (DDD-21, OD-8).** No logic change was needed.
  `submit_forgot` always renders the same page, and `reset_password_and_consume` fills a
  NULL as readily as it replaces a hash. The type change is what makes it work: before
  it, a NULL row failed to decode, and forgot-password silently sent nothing.
- **Tests.**
  - `keycloak-sso-provisioning.feature`: 11 scenarios run as 15 examples, all un-pended
    and all `@us-06`.
  - `foundry-store/tests/nullable_password_hash.rs` and `provision_federated_member.rs`.
  - `foundry-app/tests/password_less_account_doors.rs`.
  - Unit tests in `foundry-oidc` and `foundry-app::oidc`.

## Key decisions

| Decision | What it settled |
|---|---|
| **D3a** (DISCUSS addendum, US-06 AC-6.1..6.8) | Opt-in, role-gated provisioning. Unset or blank `FOUNDRY_OIDC_PROVISION_ROLE` = off = exactly D3 |
| **DDD-13** + addendum | Nullable hash, modelled as `Option` so the compiler lists every reader. The migration is one-way, so rollback is forward-only |
| **DDD-14** | Claim extraction stays in `foundry-oidc`; the decision stays in `foundry-app` |
| **DDD-15** | The link comes before the role check. Refusal order: role configured → role held → a workspace exists |
| **DDD-16 / DDD-17** | Provisioning is one transaction, safe under a race by `ON CONFLICT`, into the original workspace. An unclaimed instance refuses, so the bootstrap claim cannot be pre-empted (D5) |
| **DDD-18 / OD-7** | The greeting chain skips a name over 64 characters and never truncates it |
| **DDD-19** | A NULL hash takes the known-bad-hash arm. An early return would be a timing oracle for "this address has an SSO-only account" |
| **DDD-20** | The two new refusal reasons ride the existing `refuse()` log line; one `info` line records a provision |
| **DDD-21 / OD-8** | Reset is allowed for password-less accounts, with no logic change |
| **DDD-22 / OD-10** | Role withdrawal is **pinned, not solved**. Scenario 11 fixes today's behaviour: an account provisioned earlier keeps signing in after the role is removed. OD-10 stays **open** |
| **OD-9** | Only `realm_access.roles` counts, matched exactly and case-sensitively (confirmed by the user) |

## Steps completed

From `deliver/execution-log.json` (UTC, 2026-09-27; 26 events). `des-verify-integrity`
reports "All 4 steps have complete DES traces". Every COMMIT is `APPROVED_SKIP`.

| Step | Name | PREPARE → final GREEN | RED provenance | GREEN |
|---|---|---|---|---|
| 01-01 | Accounts may exist without a password | 20:48 → 21:09 | RED at integration level (RED_UNIT `NOT_APPLICABLE`: a DB/driver and HTTP-door contract). Two failures: the store answered **500** on a NULL hash (decode error), and **forgot-password silently sent nothing** for a password-less account. The timing assertion at the password door was mutation-checked | Migration 0016, `Option` hash end to end, `verify_against` |
| 01-02 | foundry-oidc exposes realm roles and profile names | 21:11 → 21:23 | RED_UNIT: `cargo test -p foundry-oidc`, 3 failing on assertion (roles and names not extracted; the `has_realm_role` stub returned false; the `provision_role()` stub returned `None`), 8 passing. RED_ACCEPTANCE `NOT_APPLICABLE` (no scenario; a signing issuer inside the crate would duplicate the acceptance fake) | foundry-oidc 11/11; workspace excluding acceptance 297 passed, 1 ignored; us-06 30/30 |
| 02-01 | OIDC callback provisions a member holding the provision role | 21:24 → 21:59 | RED_ACCEPTANCE: 10 examples ran, 6 passed and 4 failed. Scenario 1 and three examples of scenario 2 failed with **401, expected 303**; guards 3-7 passed. RED_UNIT: 3 `oidc` unit tests and 3 of 4 `provision_federated_member.rs` tests failed on assertion against stubs | provisioning 10/10; us-06 40/40; default lane 645/645 (first run 644/645, a us-09 sqlx `'\0'` flake, green on rerun); workspace 304 passed |
| 02-02 | Provisioned accounts at the password door, reset, and role withdrawal | 21:59 → 22:18 | Scenarios 8-11 (5 examples) were **green on the first run after un-pending**. The red for this behaviour had already been observed in 01-01's integration tests. RED_UNIT `NOT_APPLICABLE` (no defect exposed) | All 11 scenarios / 15 examples green |

**A DISTILL defect fixed in 02-01.** Scenario 2's outline placeholder `<full name>` was
never substituted, because cucumber-rs forbids spaces in outline headers. The header was
renamed to `full_name`. This is the second time the rule has bitten; see
`2026-09-14-card-drag-drop-feedback.md`. 02-01's 10 examples are scenarios 1-7 (the
ones that step un-pended); scenarios 8-11 add the other 5 in 02-02.

**Final counts:**
- provisioning: 15/15 examples;
- `us-06`: 45/45;
- default lane: 650/650;
- workspace: 304 passed.

## Refactor

Phase 3 ran as a separate L1–L4 pass in DES orchestrator mode, so it has no step entry in the
execution log. Behaviour, log strings, refusal reasons, SQL and scenario wording were
unchanged. Gates after the pass:
- workspace tests: 304 passed;
- `keycloak-sso-provisioning`: 15/15;
- `us-06`: 45/45;
- clippy, fmt and check-arch: clean.

- **L2, `signin.rs`.** The stored-hash-or-known-bad-hash block that sign-in and
  change-password each had is now one private helper, `verify_against`. That gives one
  verifier and one timing posture for both doors, and `known_bad_hash()` now has a single
  caller. L1: dropped the `let user = user_row` alias. Change-password reads through
  `find_user_by_id` (from 01-01).
- **L1, `oidc.rs`.** The private `Refusal { Refused, Internal }` became `ProvisionFailure`.
  The callback's new logic is four named units: `provision`, `judge_newcomer`,
  `greeting_name` and `log_provisioned`. The three pure ones are unit-tested directly.
- **Tests:**
  - `feature_keycloak_sso_provisioning.rs`: cookie-name constants and the helpers
    `cookie_value`, `insert_workspace` and `describe_newcomer`.
  - `provision_federated_member.rs`: `migrated_store()` merges the two setup calls;
    `account_of` added; `assert!(matches!)` → `assert_eq!`.
  - `nullable_password_hash.rs`: `assert_eq!`.
  - `password_less_account_doors.rs`: `ANY_PASSWORD`.
  - The feature file's stale "every scenario is `@pending`" header was corrected.
- **Left alone on purpose:**
  - `provision()` keeps its 4 parameters;
  - `harness.rs` `spawn_inner` keeps 5, a long parameter list that predates D3a;
  - store row mapping stays hand-written, because the crate doesn't use `FromRow`.
- **Phase 5 test code.** In `password_less_account_doors.rs`, `post_form` now sits behind
  `post_form_as` (sends session cookies, returns cookies set), and `password_hash_of` was
  added.

## Reviews

- **DISTILL review, 2026-09-27.** The acceptance-designer reviewer and the
  solution-architect reviewer both returned **NEEDS_REVISION**.
  - **5 findings accepted:**
    - US-06 and its ACs added;
    - reset designed (DDD-21);
    - the over-64 name rule decided (DDD-18);
    - role withdrawal pinned (DDD-22);
    - migration rollback stated as one-way.
  - **4 rejected:**
    - Seeding scenario 8 by SQL: fixture theater.
    - Replacing the DB-read Thens: they have no HTTP observable.
    - Tagging the timing scenario `@flaky`: it uses the same oracle as the shipped
      `us-06-signin`.
    - A concurrent-first-sign-in acceptance scenario: it would be nondeterministic, so it
      became a store test instead.
- **Roadmap review:** **APPROVED**.
- **Phase 4 adversarial review:** **APPROVED**, with no defects.

## The mutation story

The gate is per-feature, at ≥80%. The tool was cargo-mutants 25.3.1, run with
`--in-diff` over the four production files' uncommitted diff, in copy mode and never
`--in-place`.
- **Result.** 33 generated, 8 unviable, 25 viable. **24/25 killed = 96.0%, PASS.**
- **Package tests alone: 84.0% (21/25).** Three foundry-app mutants survived the
  package tests and were killed only by the acceptance lane. The lane was applied by
  hand in an `rsync` copy of the tree, because `--test-package foundry-acceptance`
  did not take effect in 25.3.1. The three are:
  - `provision → Ok(Default)`;
  - `callback → Default`;
  - `submit_change_password → Default`.
- **One accepted survivor: `OidcConfig::from_env → Ok(None)`.** `from_env` is the
  deliberately untested humble adapter: `set_var` is process-global and tests run in
  parallel. D3a adds one line to it, which reads `FOUNDRY_OIDC_PROVISION_ROLE`, and no
  test pins that name. A typo would fail safe, leaving provisioning off (link-only).
- **Hand-seeded faults**, outside the denominator, covering what cargo-mutants cannot
  mutate:
  - **H1:** a password-less account passes change-password reauth. It **survived**. This
    was a real gap: a signed-in, SSO-only session alone could set a password. It was
    **closed** by a new test,
    `a_signed_in_password_less_account_cannot_set_a_password_by_changing_it`. The test
    expects 401 for a guess, for `""` and for the old password; it also checks the hash
    stays NULL and no notification is sent. It went red under H1.
  - **H2:** the race loser (`Existing`) is refused instead of signed in. It
    **survived** and is **open**. The store half is covered by
    `concurrent_first_sign_ins_share_one_account`. The app half needs a deterministic
    interleaving, and `Store` is concrete with no port to double.
  - **H4:** provisioning into the newest workspace. It was **killed**.

  All four production-file hashes were identical before and after the mutation phase.

## Lessons and issues

1. **The type change was the reset fix.** DESIGN predicted it (DDD-21), and 01-01's red
   proved it: while `password_hash` was `String`, `submit_forgot`'s `if let
   Ok(Some(user))` swallowed the decode error. The page stayed non-enumerable, and the
   provisioned member simply never got a link. A silent, safe-looking failure is still a
   failure. Scenario 10 now guards it end to end.
2. **Cucumber-rs outline headers must be single tokens.** In DISTILL's RED run, all
   three scenario-2 examples failed at the shared 401, before the name mattered, so the
   literal `<full name>` went unnoticed. A RED that fails upstream of a placeholder
   proves nothing about the placeholder. This is the second feature to hit the rule
   (after card-drag-drop-feedback). A lint over outline headers would catch it earlier.
3. **The package-only kill rate hides the acceptance lane's work.** 84% against 96%:
   three kills needed the acceptance lane. The cargo-mutants 25.3.1 `--test-package`
   limitation should be recorded where the next per-feature gate will find it.
4. **Known infrastructure flakes** were seen during the wave, and none reproduced on a
   rerun:
   - the sqlx `Protocol('\0')` / `unknown message type '\0'` flake (02-01, us-09);
   - a testcontainers port-fetch failure;
   - `Connection reset by peer`.

   Every mutation kill was checked in its log, so none was "caught" by one of these.

## Numbers

| | |
|---|---|
| Acceptance, provisioning | 11 scenarios / 15 examples, 15/15, 0 `@pending` tags |
| `us-06` | 45/45 |
| Default lane | 650/650 |
| Workspace tests | 304 passed, 0 failed |
| DELIVER steps | 4, all GREEN; 26 DES events; integrity: all 4 steps complete |
| Mutation | 96.0% (24/25); 84.0% package-only; 1 accepted survivor; H1 closed, H2 open |
| Migration | 1 (`0016`), one-way |
| Final CI gate | 854/855 scenarios. The 1 failure is the pre-existing host-`psql` gap in `backup-verify` row counts, not D3a (3/3 in isolation) |

## Artifacts

- `docs/feature/keycloak-sso/feature-delta.md`. The increment appears as:
  - the DISCUSS US-06 addendum;
  - DESIGN `### [REF] D3a addendum (2026-09-27)`;
  - DISTILL `### [REF] D3a increment (2026-09-27)`;
  - `## Wave: DELIVER`.
- `docs/feature/keycloak-sso/deliver/`: `roadmap.json`, `execution-log.json` and
  `mutation/mutation-report.md`.
- `docs/product/architecture/brief.md` § "Federated identity is additive, never a
  migration", with its D3a addendum.
- `docs/architecture/atdd-infrastructure-policy.md`: the identity-provider fake row,
  added by the D3a DISTILL.
- `crates/foundry-store/migrations/0016_nullable_password_hash.sql`
- `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature`

## 2026-10-03 increment: OD-11

### Summary

Roadmap phase 03 (steps 03-01..03-06, approved 2026-10-03 by
nw-acceptance-designer-reviewer, `e13940a`) un-pended all 23 base
`keycloak-sso.feature` scenarios. The flow shipped in `c755003` (v0.4.0) finally has
acceptance coverage. **No production code changed:** every scenario passed against the
shipped flow once the harness drove it for real. The diff touches only
`crates/foundry-acceptance/` (the base and provisioning step modules, `world.rs`, and
23 `@pending` tags removed from the feature file, Gherkin otherwise byte-identical).

- The KNOWN RED GAP harness was replaced by `InProcHarness::spawn_with_oidc` with the
  provision role unset (D3 exactly), and a real round-trip: start, the double's
  `/authorize`, callback with the genuine state and sealed challenge cookie.
- Flag-only Whens became real HTTP (password door, bootstrap claim, sign-out), and
  scenario 23 boots the real `foundry` binary half-configured.
- **User decision 2026-10-03, scenario 19:** landing is per door: `/` for the
  password and SSO doors, `/dashboard` for the bootstrap claim, with identity
  confirmed by following the redirect.
- OD-11 is **resolved**. OD-10 stays open.

### Commits

| Commit | Step | What |
|---|---|---|
| `29a7326` | 03-01 | Walking skeleton + US-01 (5); harness wired to `spawn_with_oidc` |
| `72a76f3` | 03-02 | US-02 refusals (3); refusal Then compares with a real wrong password |
| `26ecdd9` | 03-03 | US-03 forged and stale arrivals (6) |
| `b6689e9` | 03-04 | Replay, unreachable provider, identical refusals (3) |
| `39863a6` | 03-05 | US-04 doors (3); per-door landing; same-person by user id |
| `5c8491b` | 03-06 | US-05 no provider and half-configured startup (3); closes OD-11 |
| `0c2553d` | — | F10 harness fix: the unstarted arrival is refused by the missing challenge alone |
| `4e6a1f2` | — | Refactor, test code only, oracles unchanged |

### Gates

| | |
|---|---|
| keycloak-sso lane | 38/38 (23 base + provisioning) |
| keycloak-sso-provisioning | 15/15 |
| `us-06` sign-in | 45/45 |
| Default lane | 677/677 (at 03-06) |
| check-arch | Green |
| smoke | Green 03-01..03-05; at 03-06 the workspace test gate failed only on foundry-store testcontainers start-up flakes (`PortNotExposed` / `PoolTimedOut`); foundry-store passed 25/25 binaries alone. The increment touches only `foundry-acceptance` |
| Integrity | `des-verify-integrity`: all 10 steps complete |
| Adversarial review | nw-software-crafter-reviewer: APPROVED, zero defects |
| Mutation (cargo-mutants) | Not run: no production file in the diff. Named faults stand in |
| **Full CI** | **GREEN on 4e6a1f2 (2026-10-03): exit 0, all gates green; acceptance (all tags) 903/903 scenarios, 6268/6268 steps, browser lane run** (`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`) |

### Faults

**25/25 named faults killed (F1–F25)**, each seeded alone in production code,
restored and `cmp`-verified. Variants also killed: F16b (session rotated on refusal),
F17 (`expect()` panic on discovery failure), F-landing (password door → `/dashboard`).
Four survived a first oracle and were killed only after a test was tightened:

- **F2** (fixed state/nonce): the whole-`Location` compare differed anyway because of
  PKCE; killed once the Then compared state and nonce.
- **F8** (refusal names its reason): killed once the refusal Then compared status and
  CSRF-masked body with a real wrong-password answer.
- **F18b** (no-workspace → 403): the no-workspace branch renders outside `refuse()`;
  killed once the sweep collected that arrival.
- **F10** (missing challenge cookie trusted): the scenario's world had no account, so
  the forgery was refused downstream anyway. Killed after `0c2553d` made the
  arrival one that would otherwise succeed.

### Lessons

1. **`Secure` cookies over loopback HTTP vanish silently.** A cookie-store client
   drops foundry's `Secure` cookies on plain-HTTP loopback, so several scenarios
   looked refused for the wrong reason: the arrival never carried its challenge and
   was stopped at "no challenge cookie" before the check under test. Carry cookies by
   hand, and make a refusal scenario prove it reached the guard it names.
2. **A refusal oracle that only checks forbidden phrases is too weak.** It let a
   reason-naming refusal (F8) through. Compare status and masked body with a real
   reference refusal (a wrong password) instead.
3. **A refusal can be right for the wrong reason.** F10 survived because the world
   lacked what a successful forgery needs. A guard's scenario must build the world in
   which that guard is the only thing standing.
4. **Green-on-first-run against shipped code needs fault seeding to mean anything.**
   21 of 23 scenarios were green on first run (the other two were red on the harness,
   not the product); the named faults are what showed four oracles were too weak.
5. **Whole-redirect comparisons hide the field that matters.** PKCE alone kept the
   `Location` differing under F2; compare the specific parameters.

## Follow-ups

- **OD-10: role revocation.** An account provisioned earlier keeps signing in after its
  role is withdrawn. Scenario 11 pins that, so a revocation design must change a named
  scenario.
- ~~**OD-11: un-pend the 23 base `keycloak-sso.feature` scenarios.**~~ **Closed
  2026-10-03** by phase 03 (`5c8491b`); see § "2026-10-03 increment: OD-11".
- **Refuse an empty expected nonce in `foundry-oidc` (security hardening).** Without a
  challenge the expected nonce is `""`, and an ID token with no nonce deserialises as
  `""` (`serde(default)`), so the nonce comparison passes. The challenge cookie is
  the sole guard against code injection. Refuse an empty expected nonce, with a
  scenario. Raised 2026-10-03; production unchanged.
- **Stale feature header.** `keycloak-sso.feature` line 11 says it "provisions nothing
  (D3)", true only with `FOUNDRY_OIDC_PROVISION_ROLE` unset. DISTILL-owned; left
  as-is.
- **A test for the `FOUNDRY_OIDC_PROVISION_ROLE` name.** Give `from_env` a pure lookup
  seam (e.g. `from_lookup(impl Fn(&str) -> Option<String>)`). That kills the accepted
  survivor.
- **The concurrent loser (`Existing`) path, end to end (H2).** It needs a store seam, or
  an acceptance step that inserts the row between the two queries.
- **Change-password copy for password-less accounts.** A 401 is correct, but the
  refusal copy was written for a wrong current password. Consider pointing an SSO-only
  member to forgot-password instead.
- **The empty-greeting-name refusal.** DDD-18 calls it "unreachable", and nothing
  exercises it.
- **Decide whether to keep** the identity-provider row that the D3a DISTILL added to
  `docs/architecture/atdd-infrastructure-policy.md`.
- **Outcome IDs.** The keycloak-sso rows (OUT-1..5, and OUT-6 proposed by D3a) were never
  registered, because the `nwave-ai outcomes` CLI was broken. Since then, other features
  have used OUT-1..OUT-16 in `docs/product/outcomes/registry.yaml`. These rows need new
  IDs when they are registered.
