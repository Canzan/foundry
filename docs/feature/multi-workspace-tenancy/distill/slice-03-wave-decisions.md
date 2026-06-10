# Multi-Workspace Tenancy — Slice 3 (API + Machine-Token + Session-Resolution) DISTILL Wave Decisions

> Sentinel (nw-acceptance-designer), DISTILL wave, SLICE 3 ONLY (propagate the
> isolation boundary to the JSON `/api/v1` remaining surfaces + machine-token +
> the sign-in/session-resolution CONTRACT). Legacy per-feature layout; trunk-based
> (commit to `main`, no branch/PR). Scenarios for Slices 1-2, 4-6 are NOT authored
> here. Slice 1 (API issues READ scoped by `token.workspace_id`) and Slice 2 (web
> session resolution + switcher + uniform-404 + the LAYER-1e guard) are the
> dependencies this slice builds on — referenced, not re-authored.

## Reconciliation HARD GATE result

**Reconciliation passed — 0 contradictions.**

Read: `discuss/wave-decisions.md` (ratified OD-2 multi-membership, OD-3 instance
super-admin), `discuss/nfrs.md`, `discuss/stories.md` (US-MWT03 + US-MWT04 + the
cross-cutting system constraints), `slices/slice-03-api-and-auth-boundary.md` (the
slice contract — the "Done when" the scenarios satisfy), `design/architecture.md`,
`design/adr-001-request-workspace-resolution.md` (token.workspace_id = the
authoritative API acting workspace; session leg), `design/adr-003-non-enumerability-contract.md`
(uniform foreign-id ≡ missing-id; API = the shipped `status_for` 404 envelope;
foreign-jti revoke already non-enumerable `NotFound`), `design/adr-005-multimembership-signin-selection.md`
(single auto / multi explicit / none → fail-closed), `distill/slice-01-wave-decisions.md`,
`distill/slice-02-wave-decisions.md`.

| DISCUSS decision | DESIGN position | Slice-3 relevance | Verdict |
|---|---|---|---|
| OD-1 shared-schema + `workspace_id` | ADR-003 ratify shared-schema | API reads/writes + token list/revoke scoped by `workspace_id` | consistent |
| OD-2 multi-membership (RATIFIED) | ADR-005 session active-workspace + `resolve_active_workspace` | the session-resolution-contract scenarios (7, 8, 9) | consistent |
| DM2 / NFR-MWT-SEC-02 isolation fail-closed + non-enumerable | ADR-003 generalize `find_*_in_workspace`→None; API = shipped 404 JSON envelope; foreign-jti revoke = shipped non-enumerable `NotFound` | the API refusal core (3, 4, 5, 6) | consistent |
| NFR-MWT-SEC-05 machine-token binding IS the acting workspace | ADR-001 API leg: `Principal::Machine{workspace_id}` from the token's registry row (`foundry-api/src/lib.rs`) | the confinement core (1, 2, 3, 4, 5, 6) | consistent |
| NFR-MWT-SEC-03 fail-closed when none resolvable | ADR-005: 0 memberships → `resolve_active_workspace` returns `None` → refuse | scenario 9 | consistent |
| DM8 / NFR-MWT-TEST-01 residuals IN scope (real fixtures replace synthetic uuids) | feature is the named trigger to close UI-1 | scenarios 5, 6 (token list/revoke residual closure) | consistent — **RESOLVED this slice** |
| Carried invariant: shipped verify path unchanged | ADR-003 boundary clause: this slice scopes WHICH workspace, not HOW a token is verified | scenario 10 (verify-path regression) | consistent |

**Nuance surfaced (NOT a contradiction):** ADR-001 records that the web/session
leg's `resolve_active_workspace` membership resolution (ADR-005) and the
`/workspace/switch` switcher were SHIPPED by Slices 1-2 DELIVER (confirmed in code:
`foundry-store/src/lib.rs::resolve_active_workspace` + `::set_active_workspace`,
`foundry-app/src/session.rs::submit_switch`, the `/workspace/switch` route in
`foundry-app/src/lib.rs:296`, and `signin.rs` already failing closed on `None`). So
US-MWT04's session-resolution CONTRACT is green-by-inheritance at the store/sign-in
seam; Slice 3's job is to ASSERT that contract directly (resolution yields exactly
one + fail-closed when none) — distinct from Slice 2's web SWITCH scenarios that
exercise the same seam through the board UI. Authored that way per ADR-005 + the
slice contract. No DISCUSS↔DESIGN↔DEVOPS opposition. Gate passed.

## API / auth surfaces covered (and green-by-inheritance vs needing DELIVER)

| Surface | Path / seam | Slice-3 scenario(s) | Status |
|---|---|---|---|
| Issue READ (slice-1 path, on the slice-3 fixture) | `GET /api/v1/.../issues` scoped by `token.workspace_id` | 2 | green-by-inheritance behind the `0002` gate |
| Issue WRITE (NEW confinement surface) | `POST /api/v1/.../issues` → `create_issue` + `insert_issue_with_outbox` bound to the acting workspace | 1 (WS), 4 | shipped scoping; green-by-inheritance behind `0002` |
| Cross-tenant READ refusal | `GET .../issues` foreign project vs never-existed (ADR-003 uniform 404) | 3 | shipped `find_*_in_workspace`→None; green behind `0002` |
| Cross-tenant WRITE refusal | `POST .../issues` foreign project vs never-existed | 4 | shipped scoping; green behind `0002` |
| Token LIST confined (residual closure) | `GET .../tokens` → `list_tokens(principal.workspace_id())` | 5 | shipped + 100%-mutation-hardened; green behind `0002` |
| Token REVOKE confined (residual closure, KEY item) | `DELETE .../tokens/{jti}` → `revoke_token`: `row.workspace_id != principal.workspace_id() ⇒ NotFound` | 6 | shipped + mutation-hardened; green behind `0002` |
| Session resolution — single-membership auto | `Store::resolve_active_workspace` (ADR-005) | 7 | shipped by slices 1-2; contract asserted directly |
| Session resolution — multi-membership → chosen one | `set_active_workspace` (persisted `active_workspace_id`) + `resolve_active_workspace` | 8 | shipped by slices 1-2 (02-05) |
| Session resolution — fail-closed when none | `resolve_active_workspace` → `None` | 9 | shipped by slices 1-2 |
| Verify-path-unchanged regression | per-request `jti` denylist + EdDSA/`iss`/`aud` pinning | 10 | shipped; regression-guarded under multi-workspace |

**The ONLY genuinely-new RED edge in this slice is the `0002` migration** (drop
`uniq_one_workspace`), shared with Slices 1-2 — once it ships, every confinement
scenario is green-by-inheritance because the per-table `workspace_id` scoping, the
`list_tokens`/`revoke_token` workspace confinement, and the session resolution seam
are all ALREADY shipped + mutation-hardened. They have simply never been exercised
under a genuinely-coexisting second workspace. There is NO new production module to
scaffold; the only DELIVER prerequisite specific to behaviour is verifying those
shipped paths hold under two real tenants (the whole point of the feature).

## Residual closure — token list/revoke use REAL two-workspace fixtures

**CONFIRMED.** The `token-management-api` feature tests cross-workspace
non-enumerability with a SYNTHETIC random uuid as the "foreign" jti:
`feature_token_management_api.rs::credential_in_another_workspace` records
`world.mt_foreign_jti = Some(uuid::Uuid::now_v7())` with the explicit comment
"Single-workspace model (uniq_one_workspace …): a real foreign workspace row is not
insertable." That was the accepted residual (UI-1 / `docs/evolution/2026-06-08-token-management-api.md`).

Slice-3 scenarios 5 + 6 CONVERT that synthetic residual to real fixtures:
- Scenario 5 (`a managed token "globex-ci" exists in workspace "Globex"`) seeds a
  REAL `machine_tokens` row in the REAL Globex workspace, bound to Globex's real
  admin. An Acme-bound token lists tokens and the assertion is that `acme-ci` IS
  present and `globex-ci` is NOT — proving `list_tokens(principal.workspace_id())`
  confines to the acting workspace under a real cross-tenant fixture.
- Scenario 6 (the KEY item) revokes the REAL Globex jti with the Acme token; it is
  refused 404 byte-identically to a never-existed jti, AND `globex-ci`'s
  `revoked_at` is asserted STILL NULL (the Globex token stays active). This proves
  `revoke_token`'s `row.workspace_id != principal.workspace_id() ⇒ NotFound`
  against a real foreign row, not a random uuid that simply isn't in the registry.

This closes NFR-MWT-TEST-01 / DM8 for the API token surface.

## Uniform refusal status / shape decision (confirmed with ADR-003)

- **API cross-tenant RESOURCE reach (issue read/write) → the SHIPPED `status_for`
  404 JSON envelope, byte-identical (status + body) to a never-existed id.** No 403
  for cross-tenant resource access (a 403-vs-404 difference is an enumeration
  oracle). Generalises `find_*_in_workspace → None` (ADR-003 option (b)).
- **API foreign-jti revoke → the SHIPPED non-enumerable `NotFound` 404** byte-
  identical to a never-existed jti (`tokens.rs:267`, reused as-is).
- **Timing equivalence is structural** — foreign-id and missing-id execute the SAME
  `WHERE id AND workspace_id` query, so they share a timing profile by construction
  (ADR-003). Slice 3 asserts status + body identity; the timing/shape adversarial
  matrix across ALL surfaces is Slice 4.
- **The shipped bearer 401 + the non-enumerable sign-in error are UNCHANGED** —
  ADR-003 boundary clause. Scenario 10 regression-guards the 401 verify path.

## Tier classification

**Tier A only.** LAYER 3 (real Postgres via testcontainers + per-scenario schema +
real HTTP via the in-process `InProcHarness`/`reqwest`, under the real EdDSA
verifier + jti denylist; the session-resolution scenarios drive the SHIPPED
`resolve_active_workspace` store seam directly). Per Mandates 9 + 11: example-based;
every sad/evil-user path enumerated explicitly; NO PBT machinery. Per Mandate 10:
Tier B (state-machine PBT, in-memory doubles) is NOT added — the journey runs at
layer 3 with real I/O, and although the session-resolution contract has 3 cases,
the input space is not domain-rich (fixed Acme/Globex personas), so Tier A examples
cover it. Per Mandate 8: layer-3 uses traditional assertions over port-exposed
observables (listed issue keys, listed token labels, HTTP refusal status + body
identity, post-write workspace-scoped DB row presence, post-revoke `revoked_at`
state, the resolved workspace id) — the state-delta universe-guard is the
layers-1-3 requirement satisfied by traditional port-observable assertions at this
layer per the Layered Test Discipline table (matching slices 1-2; no
`state_delta.rs` Rust port exists — Python is the canonical pilot).

## Scenario list + tags

File: `crates/foundry-acceptance/tests/features/us-mwt-slice-03-api-auth-boundary.feature`

| # | Scenario | Story | Tags | Class |
|---|----------|-------|------|-------|
| 1 | A workspace-bound token's write lands only in its own workspace | US-MWT03 | `@walking_skeleton @wiring_e2e @us-mwt03` | happy (WRITE confinement, core hypothesis) |
| 2 | A workspace-bound token reads only its own workspace's issues over the API | US-MWT03 | `@us-mwt03 @pending` | happy (READ confinement) |
| 3 | A cross-workspace API read is refused non-enumerably | US-MWT03 | `@us-mwt03 @error @pending` | evil-user (read refusal core) |
| 4 | A cross-workspace API write is refused non-enumerably | US-MWT03 | `@us-mwt03 @error @pending` | evil-user (write refusal core) |
| 5 | A workspace-bound token lists only its own workspace's tokens | US-MWT03 | `@us-mwt03 @error @pending` | evil-user (token-list residual closure) |
| 6 | A workspace-bound token cannot revoke another workspace's token | US-MWT03 | `@us-mwt03 @error @pending` | evil-user (token-revoke residual closure, KEY) |
| 7 | A single-membership session resolves to exactly one workspace automatically | US-MWT04 | `@us-mwt04 @pending` | happy (resolution contract) |
| 8 | A multi-membership session resolves to exactly the chosen workspace | US-MWT04 | `@us-mwt04 @pending` | multi-membership (resolution contract) |
| 9 | A session that resolves to no workspace is refused, not defaulted | US-MWT04 | `@us-mwt04 @error @pending` | error (fail-closed) |
| 10 | The shipped token verify path and jti denylist are unchanged under multi-workspace | US-MWT03 | `@us-mwt03 @error @verify-path-unchanged @pending` | regression (invariant) |

Feature-level tags: `@multi-workspace-tenancy @mwt-slice-03 @real-io @driving_adapter`.

- **Error/evil-user ratio**: 6 of 10 (scenarios 3, 4, 5, 6, 9, 10) = **60%**
  (exceeds the 40% bar).
- **Story coverage**: US-MWT03 (1-6, 10 — all five ACs: API resolves from token
  binding; cross-tenant refused identically; revoke/list confined; verify path
  unchanged; real Acme-bound token vs real Globex resources) + US-MWT04 (7, 8, 9 —
  all three ACs: single auto, multi explicit, none fail-closed). Both slice-3
  stories fully covered. All scenarios use REAL Acme/Globex fixtures (no synthetic
  uuids).
- **Walking skeleton**: exactly ONE (`@walking_skeleton`, scenario 1) — demo-able:
  "an Acme-bound token files an issue and it lands in Acme and only Acme." Active
  (un-skipped RED). Scenarios 2-10 are `@pending` per one-at-a-time DELIVER.

## Adapter coverage table (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Postgres per-table `workspace_id` issue READ scoping (`list_issues_by_project`) | YES | 2, 3 |
| Postgres issue WRITE scoped by acting `workspace_id` (`create_issue` / `insert_issue_with_outbox`) | YES | 1, 4 (+ post-write workspace-scoped row-count assertion) |
| `list_tokens(principal.workspace_id())` workspace-scoped token list | YES | 5 (real Acme + real Globex token rows) |
| `revoke_token` cross-tenant `NotFound` (`row.workspace_id != principal.workspace_id()`) | YES | 6 (real Globex jti; Globex token asserted still active) |
| `machine_tokens` registry + EdDSA verify (the `token.workspace_id` resolution seam) | YES | 1-6 (real bearers minted bound to a workspace) |
| per-request `jti` denylist + EdDSA/`iss`/`aud` pinning (verify path) | YES | 10 (revoked-jti 401 + disallowed-alg 401) |
| `Store::resolve_active_workspace` session-resolution seam (ADR-005) | YES | 7, 8, 9 (single / multi-chosen / none) |
| `Store::set_active_workspace` persisted-active (the 02-05 switcher seam) | YES | 8 |
| JSON API driving adapter (`foundry-api` over real HTTP) | YES | 1-6, 10 (driving-adapter coverage per RCA-fix P1 — real bearer HTTP, not a direct service call) |
| The `0002` forward-only migration (drop `uniq_one_workspace`) | YES | Background of every scenario (second workspace insert) |

Zero `NO — MISSING` rows. All driven adapters in slice-3 scope are exercised with
real I/O. Mechanism per the Project Infrastructure Policy
(`docs/architecture/atdd-infrastructure-policy.md`) — all ports already recorded
(HTTP API via `spawn_app`/`reqwest`; PgPool via testcontainers + per-scenario
schema; EdDSA fixed test keypair). **No policy rows added this run**
(`--policy=inherit`, every slice-3 port present).

## NEW steps/fixtures vs reused

**NEW (slice-3 phrases — globally-unique cucumber-rs step text):**
- Given `a managed token "<label>" exists in workspace "<ws>"` — seeds a REAL
  `machine_tokens` row in the NAMED workspace bound to its admin (the residual-
  closure fixture; the Globex variant is the real foreign target).
- Given `that credential has been revoked` — mints+registers a fresh known-jti
  Acme bearer, revokes its registry row (the verify-path regression target).
- Given `"<email>" belongs to exactly one workspace "<ws>"` — single-membership
  precondition (asserts count == 1, records the expected resolution target).
- Given `"<email>" has chosen "<ws>" as their active workspace` — persists
  `active_workspace_id` via the shipped `set_active_workspace` (multi-membership).
- Given `"<email>" belongs to no workspace` — the fail-closed edge (bare user, no
  membership row).
- When: `the Acme-bound credential files issue "<t>" in the "<p>" project over the
  API` (+ `… by its real address` foreign variant + `… in a project that never
  existed`); `the Acme-bound credential lists the "<p>" project's issues over the
  API by its real address` (+ `… that never existed`); `the Acme-bound credential
  lists the workspace's tokens over the API`; `the Acme-bound credential revokes
  the "<ws>" token "<label>" over the API` (+ `… a token id that exists nowhere`);
  `(his|her|their) session's acting workspace is resolved`; `the revoked credential
  lists the "<p>" project's issues as data`.
- Then: `the write is reported as created` / `the new issue exists only in "<ws>"`
  / `no issue was created in "<ws>"` / `the two API responses are refused
  identically` / `nothing in the API response reveals the "<ws>" project exists` /
  `the token list contains "<label>"` / `the token list does not contain "<label>"`
  / `the two API revoke responses are refused identically as not found` / `the
  "<ws>" token "<label>" remains active` / `the session resolves to exactly the
  workspace "<ws>"` / `no workspace choice was required` / `(her|his|their) session
  is scoped to exactly one workspace` / `no workspace is resolved` / `the session
  is not scoped to any workspace` / `the request is refused as unauthorized by the
  verify path` / `a credential signed with a disallowed algorithm is also refused
  as unauthorized`.
- World fields added to `src/world.rs` (8 `mwt3_*` slots): `mwt3_token_jti_by_label`,
  `mwt3_revoked_bearer`, `mwt3_first_refusal_body`, `mwt3_first_refusal_status`,
  `mwt3_resolution_user`, `mwt3_expected_workspace`, `mwt3_resolved_workspace`,
  `mwt3_resolution_ran`.

**REUSED (verbatim registered step text — bound by exact regex match, NOT
re-declared; cucumber-rs requires globally-unique step text):**
- The two-workspace SEED Background — slice-1's registered steps (`workspace "…"
  exists with admin "…"`, `"…" has a member … with project … prefix …`, `the "…"
  project "…" has issues …-1 and …-2`) + `a machine credential is bound to "…" in
  workspace "…"`. The coexistence fixture is slice-1's; slice 3 reuses it and adds
  only the new API/auth surface.
- The issues-list When/Then — slice-1's `the Acme-bound credential lists the "<p>"
  project's issues as data`, `the answer lists only the "…" issues …-N and …-M`,
  `no "…" issue appears in the answer`, `the request is refused` — reused for
  scenarios 2 + 10's setup.
- The multi-membership cross-seed — slice-2's `"<email>" is also a member of "<ws>"
  in team "<team>" with project "<project>" prefix "<prefix>"` (scenario 8).
- `InProcHarness::spawn` + per-scenario schema + the shared testcontainers PG16
  container; the additive `ensure_harness` pattern (mirrors slices 1-2 — never
  resets after the first spawn, so the Background's two workspaces survive).
- `Store::insert_machine_token`, `Store::revoke_machine_token`,
  `Store::resolve_active_workspace`, `Store::set_active_workspace`,
  `foundry_auth::test_keys::signer()` + `MachineTokenClaims`,
  `foundry_auth::hash_password`.

Deliberately NOT reused: a fresh harness reset on subsequent steps (would discard
the first workspace — same hazard slices 1-2 document). The slice-3 bearer-mint
helper is self-contained (mirrors `feature_token_management_api::mint_bearer`) so
it does not depend on slice-1's private fns.

## Scaffold inventory (Mandate 7 / RED-ready)

- `.feature`: `crates/foundry-acceptance/tests/features/us-mwt-slice-03-api-auth-boundary.feature`
  (10 scenarios; #1 active `@walking_skeleton`, #2-10 `@pending` per one-at-a-time).
- Steps: `crates/foundry-acceptance/src/steps/feature_mwt_slice_03_api_auth_boundary.rs`
  (registered in `src/lib.rs` `pub mod steps {…}` + force-linked via
  `use … as _feature_mwt_s03` in `tests/acceptance.rs`).
- World fields added to `src/world.rs` (8 `mwt3_*` slots).
- **No production-source scaffold stub needed**: the production surface this slice
  requires (the `0002` migration dropping `uniq_one_workspace`, shared with slices
  1-2) is a DELIVER-authored migration file, not a Rust module that step-defs
  import. The remaining scoping/resolution paths are SHIPPED. So there is no module
  to stub. The crate COMPILES clean, so the test is RED-not-BROKEN by construction.

**Gates run this slice:**
- `cargo test -p foundry-acceptance --no-run` → Finished (compiles, RED-not-BROKEN).
- `cargo fmt --all -- --check` → clean (exit 0).
- `cargo clippy -p foundry-acceptance --all-targets --release -- -D warnings` →
  clean (exit 0, 0 warnings).

## Test placement + precedent

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/*.rs` — matches
EVERY prior feature (us-w05a, us-tma, us-mt0x, us-0x, us-mwt-slice-01/02). Feature
filename `us-mwt-slice-03-api-auth-boundary.feature` + step module
`feature_mwt_slice_03_api_auth_boundary.rs` mirror the slice-named slice-1/2
precedent.

## Pre-requisites — what DELIVER must build vs what is green-by-inheritance

**DELIVER must build (the single RED edge, shared with slices 1-2):**
1. **`0002_multi_workspace.sql`** (ADR-006): `DROP INDEX uniq_one_workspace;` so the
   Background's second `workspaces` row can exist. If slices 1-2 already shipped
   `0002`, this is satisfied and EVERY slice-3 scenario is green-by-inheritance.

**Green-by-inheritance (shipped + mutation-hardened — slice 3 PROVES under two real tenants):**
2. `token.workspace_id` → `Principal::Machine{workspace_id}` resolution
   (`foundry-api/src/lib.rs`, ADR-001 API leg) — the acting workspace for `/api/v1`.
3. Issue WRITE scoping (`create_issue` → `insert_issue_with_outbox` bound to the
   acting workspace) and READ scoping (`list_issues_by_project`).
4. `list_tokens(principal.workspace_id())` workspace-scoped list + `revoke_token`'s
   `row.workspace_id != principal.workspace_id() ⇒ NotFound` (`tokens.rs`, 100%
   mutation-hardened) — the residual-closure behaviour.
5. `Store::resolve_active_workspace` (ADR-005: single auto / multi persisted-active
   / zero → `None` fail-closed) + `Store::set_active_workspace` + the
   `/workspace/switch` route (shipped by slices 1-2 DELIVER).
6. The shipped verify path: per-request `jti` denylist + EdDSA/`iss`/`aud` pinning
   (`token_auth::authenticate`) — unchanged; scenario 10 regression-guards it.

## Fail-for-the-right-reason expectation (per scenario)

The crate compiles clean (no import/collection error → not BROKEN). At runtime,
against the real testcontainers PG16, every scenario reds for
MISSING_FUNCTIONALITY (the `0002` guard drop), then — once `0002` ships — proves
the shipped behaviour under two real tenants:

| # | RED cause (the genuine missing functionality) |
|---|---|
| 1 | Background's 2nd `INSERT INTO workspaces` fails on `uniq_one_workspace` (until `0002`); once 2 coexist, the Acme-bound write must land in Acme and create no Globex row — green via the shipped `create_issue` acting-workspace binding. |
| 2 | Same Background red; once 2 coexist, the Acme token's list must contain only ACME-* — green via the shipped READ scoping. |
| 3 | Same Background red; once 2 coexist, the Acme token reaching a REAL Globex project must 404 identically to a never-existed one — green via the shipped `find_*_in_workspace`→None / 404 envelope. |
| 4 | Same Background red; once 2 coexist, the Acme token writing into a REAL Globex project must 404 identically AND create no Globex row — green via the shipped acting-workspace write scoping. |
| 5 | Same Background red; once 2 coexist, the Acme token's token list must contain `acme-ci` and NOT the real `globex-ci` — green via the shipped `list_tokens(workspace_id)`. (Residual closure — replaces the synthetic-uuid `us-tma` proof.) |
| 6 | Same Background red; once 2 coexist, revoking the REAL Globex jti must 404 identically to a never-existed jti AND leave `globex-ci` active — green via the shipped `revoke_token` cross-tenant `NotFound`. (Residual closure, KEY item.) |
| 7 | Same Background red; once 2 coexist, marco (one membership) must resolve to exactly Acme with no choice step — green via the shipped `resolve_active_workspace`. |
| 8 | Same Background red; once 2 coexist, dana (member of both, chose Globex) must resolve to exactly Globex — green via `set_active_workspace` + `resolve_active_workspace`. |
| 9 | Same Background red; once 2 coexist, an evicted (no-membership) user must resolve to NO workspace (fail-closed) — green via `resolve_active_workspace` returning `None`. |
| 10 | Same Background red; once 2 coexist, a revoked credential's next `/api/v1` call must be 401 (jti denylist) AND a disallowed-alg credential must be 401 (EdDSA pinning) — green via the unchanged shipped verify path. |

This is the RED-phase entry signal DELIVER reads at PREPARE (ADR-025 D2). No
scenario reds for a fixture/import/setup reason — the genuine missing functionality
is the `0002` guard drop (shared with slices 1-2); everything else is the shipped
boundary proven under real two-workspace fixtures.

## Scope confirmation

**SLICE 3 ONLY.** The JSON `/api/v1` remaining surfaces (issue WRITE; token
list/revoke) + machine-token confinement + the sign-in/session-resolution CONTRACT
(US-MWT03 + US-MWT04). The full uniform non-enumerability matrix across ALL surfaces
+ the adversarial timing/shape matrix (Slice 4), migration-as-guarantee (Slice 5),
and provisioning (Slice 6) are explicitly OUT — not authored here. Slice 1 (API
issues READ) and Slice 2 (web session resolution + switcher + uniform-404 + the
LAYER-1e guard) are NOT re-authored — referenced as dependencies.

## Upstream issues

**None.** No contradiction or production gap found. Every slice-3 `/api/v1`
surface is already workspace-scoped (issue read/write via the acting-workspace
binding; token list/revoke via `principal.workspace_id()`), and the session-
resolution seam (`resolve_active_workspace`) already implements ADR-005's
single/multi/none contract fail-closed. There is no un-scoped `/api/v1` path that
could not be fed the acting workspace. No `distill/slice-03-upstream-issues.md` is
written.
