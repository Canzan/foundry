# Multi-Workspace Tenancy — Slice 2 (Web-Tier Boundary) DISTILL Wave Decisions

> Sentinel (nw-acceptance-designer), DISTILL wave, SLICE 2 ONLY (tenant-scoped
> authz + non-enumerable refusal on the WEB htmx tier). Legacy per-feature layout;
> trunk-based (commit to `main`, no branch/PR). Scenarios for Slices 1, 3-6 are
> NOT authored here. Slice 1 (the API-leg coexistence walking skeleton) is the
> dependency this slice builds on.

## Reconciliation HARD GATE result

**Reconciliation passed — 0 contradictions.**

Read: `discuss/wave-decisions.md` (ratified OD-2 multi-membership, OD-3 instance
super-admin), `discuss/nfrs.md`, `discuss/stories.md` (US-MWT02 + the cross-cutting
system constraints), `slices/slice-02-web-tier-boundary.md` (the slice contract),
`design/architecture.md`, `design/wave-decisions.md`, `design/adr-002` (ActingWorkspace
seam), `design/adr-003` (non-enumerability contract), `design/adr-005` (multi-membership
sign-in selection), `distill/slice-01-wave-decisions.md`.

| DISCUSS decision | DESIGN position | Slice-2 relevance | Verdict |
|---|---|---|---|
| OD-1 shared-schema + `workspace_id` | DD-MWT-03 / ADR-003 ratify shared-schema | web reads/writes scoped by `workspace_id` | consistent |
| OD-2 multi-membership (RATIFIED) | DD-MWT-05 / ADR-005 session active-workspace + switcher | the switcher scenarios (8, 9) | consistent |
| DM2 isolation fail-closed + non-enumerable at the scoping seam | DD-MWT-03 / ADR-003 generalize `find_*_in_workspace`→None; web=404 page; cross-tenant never 403 | the non-enumerable refusal core (4, 5, 6, 7) | consistent |
| NFR-MWT-SEC-04 per-tenant authority does not cross | shipped `is_workspace_admin(acting_ws, user)` reused | admin-cannot-cross (7) | consistent |
| ADR-002 `ActingWorkspace` newtype + check-arch tenant-scoping rule | newtype lands Slice 1; the guard RULE lands WITH Slice 2 | the structural pre-requisite DELIVER builds | consistent |
| OD-MWT-D6 cross-tenant refusal status (per-surface) | **web = 404 page**; API = JSON 404 envelope | the refusal-status decision below | consistent — **RESOLVED** |

**Nuance surfaced (NOT a contradiction):** ADR-003 is explicit that a CROSS-tenant
RESOURCE reach is non-enumerable **404** on the web, while an INTRA-workspace authz
failure keeps its shipped shape. For US-MWT02's "an admin of A cannot manage B"
scenario, the actor (Priya) acts on Acme and reaches a Globex credential — that is a
CROSS-tenant reach, so it is the non-enumerable **404**, NOT a 403. The shipped
`/admin/tokens` surface already collapses non-admin / missing / foreign jti to the
SAME non-enumerable 404 (`admin_tokens.rs:48`, NFR-MT-SEC-03), so this is the shipped
idiom proven under a genuinely-coexisting second workspace. Scenarios are authored
that way per ADR-003's boundary clause.

No DISCUSS↔DESIGN↔DEVOPS opposition. Gate passed.

## Uniform refusal status / shape decision (confirmed with ADR-003)

- **Web cross-tenant RESOURCE reach (board, issue, write) → HTTP 404 not-found page,
  IDENTICAL (status + page body shape) to a never-existed id.** No 403 for
  cross-tenant resource access (a 403-vs-404 difference is an enumeration oracle).
  Generalises the shipped `find_*_in_workspace → None` idiom (ADR-003 option (b)).
- **Web cross-tenant ADMIN action (revoke a foreign credential) → the SAME
  non-enumerable 404**, not a 403/200 (`admin_tokens.rs:48` already does this for
  non-admin/missing/foreign jti). Confirms ADR-003 + NFR-MT-SEC-03.
- **Timing equivalence is structural**, not a constant-time hack: foreign-id and
  missing-id execute the SAME `WHERE id AND workspace_id` query, so they share a
  timing profile by construction (ADR-003). Slice 2 asserts status + body identity;
  the timing/shape adversarial matrix across ALL surfaces is Slice 4.

## Tier classification

**Tier A only.** LAYER 3 (real Postgres via testcontainers + per-scenario schema +
real HTTP via the in-process `InProcHarness`/`reqwest`, under the production
session + double-submit CSRF layers). Per Mandates 9 + 11: example-based; every
sad/evil-user path enumerated explicitly; NO PBT machinery. Per Mandate 10: Tier B
(state-machine PBT, in-memory doubles) is NOT added — the journey runs at layer 3
with real I/O, and although the multi-membership switch is a 2-scenario chain, the
input space is not domain-rich (fixed Acme/Globex personas), so Tier A examples
cover it. Per Mandate 8: layer-3 uses traditional assertions over port-exposed web
observables (rendered page substrings, HTTP refusal status, post-write
workspace-scoped DB row presence) — the state-delta universe-guard is the layers-1-3
requirement satisfied by traditional port-observable assertions at this layer per
the Layered Test Discipline table (matching slice-1's precedent; no `state_delta.rs`
Rust port exists — Python is the canonical pilot).

## Web paths covered (read / write / admin / refusal / switcher)

| Class | Web path | Scenario(s) |
|---|---|---|
| READ (board) | `GET /team/{team}/project/{project}` | 1 (walking skeleton), 4 (foreign refusal) |
| READ (issue detail) | `GET /team/{team}/project/{project}/issues/{n}` | 2, 5 (foreign refusal) |
| WRITE (file issue) | `POST /team/{team}/project/{project}/issues` | 3, 6 (foreign refusal) |
| ADMIN (gated) | `POST /admin/tokens/{jti}/revoke` | 7 (admin-cannot-cross) |
| SWITCHER (new) | `POST /workspace/switch` (DELIVER adds, ADR-005) | 9 |

Representative web read paths chosen (board + issue detail) and write paths
(file-issue) — the highest-traffic tenant-scoped surfaces. Comment/attachment/
state-change writes share the SAME `find_team_by_slug(workspace_id, …)` →
`is_team_member` scoping chain (`projects.rs`, `issues.rs:128-172`,
`attachments.rs`), so proving the boundary on board + issue + file-issue exercises
the identical scoping seam; enumerating every write path is Slice 4's adversarial
matrix, not slice 2's representative proof.

## Scenario list + tags

File: `crates/foundry-acceptance/tests/features/us-mwt-slice-02-web-boundary.feature`

| # | Scenario | Tags | Class |
|---|----------|------|-------|
| 1 | A member sees only their own workspace's board on the web | `@walking_skeleton @wiring_e2e` | happy (core hypothesis) |
| 2 | A member reads only their own workspace's issue detail on the web | `@pending` | happy (read) |
| 3 | A member's write affects only their own workspace on the web | `@pending` | happy (write) |
| 4 | Reaching another workspace's board by its real address is refused non-enumerably | `@pending @error` | evil-user (read refusal core) |
| 5 | Reaching another workspace's issue by its real address is refused non-enumerably | `@pending @error` | evil-user (read refusal core) |
| 6 | Writing into another workspace's project is refused non-enumerably | `@pending @error` | evil-user (write refusal core) |
| 7 | An admin of one workspace cannot manage another's credentials on the web | `@pending @error` | evil-user (admin-cross) |
| 8 | A multi-membership user acts on exactly their active workspace on the web | `@pending` | multi-membership (OD-2) |
| 9 | Switching the active workspace changes which workspace's data is shown | `@pending` | multi-membership switch (ADR-005) |

Feature-level tags: `@multi-workspace-tenancy @mwt-slice-02 @real-io @driving_adapter @us-mwt02`.

- **Error/evil-user ratio**: 4 of 9 (scenarios 4-7) = **44%** (exceeds the 40% bar).
- **Story coverage**: US-MWT02 — all four ACs covered (scoped reads/writes 1-3;
  non-enumerable refusal 4-6; admin-gate 7; multi-membership 8-9 satisfy OD-2's web
  expression). All scenarios use REAL Acme/Globex fixtures (no synthetic uuids).
- **Walking skeleton**: exactly ONE (`@walking_skeleton`, scenario 1) — demo-able:
  "a member of Acme, browsing the web, sees only Acme's board." Active (un-skipped).
- **Pillar 2 (chained narrative)**: the Background `Given`s are reused verbatim from
  slice-1's registered step text (workspace/member/issue seeds), and the multi-
  membership scenarios (8, 9) reuse the slice-1 member+team+project seed via a
  delegating `is also a member of …` step — no copy-pasted fixture setup.

## Adapter coverage table (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Postgres per-table `workspace_id` scoping (board/issue READ via `find_team_by_slug(ws,…)` → `is_team_member` → `find_project_by_slug`) | YES | 1, 2, 4, 5 |
| Postgres issue WRITE scoped by acting `workspace_id` (`submit_create`) | YES | 3, 6 (+ post-write workspace-scoped row-count assertion) |
| `is_workspace_admin(acting_ws, user)` authz gate (web `/admin/tokens` revoke) | YES | 7 |
| `machine_tokens` registry (foreign credential seed + non-enumerable revoke 404) | YES | 7 |
| tower-sessions Postgres store + double-submit CSRF (real signed-in cookie path) | YES | 1-3, 6-9 (every signed-in web GET/POST) |
| Session active-workspace resolution + switcher (`SessionUser` EXTEND, ADR-005) | YES | 8, 9 (the `/workspace/switch` re-stamp) |
| htmx web driving adapter (`foundry-app` over real HTTP) | YES | 1-9 (driving-adapter coverage per RCA-fix P1 — real signed-in HTTP, not a direct service call) |

Zero `NO — MISSING` rows. All driven adapters in slice-2 scope are exercised with
real I/O. Mechanism per the Project Infrastructure Policy
(`docs/architecture/atdd-infrastructure-policy.md`) — all ports already recorded
(HTTP API via `spawn_app`/`reqwest`; PgPool via testcontainers + per-scenario schema;
tower-sessions Postgres store; EdDSA fixed test keypair for the seeded credential).
**No policy rows added this run** (`--policy=inherit`, every slice-2 port present).

## NEW steps/fixtures vs reused

**NEW (slice-2 web phrases — globally-unique cucumber-rs step text):**
- `"<email>" is also a member of "<ws>" in team "<team>" with project "<project>" prefix "<prefix>"`
  — additive cross-membership seed (OD-2); DELEGATES to slice-1's
  `workspace_has_member_team_project` (made `pub` this run) so the route is recorded
  workspace-scoped, then sets the web password.
- `"<email>" is signed in on the web acting on workspace "<ws>"` — records the
  signed-in persona + the INTENDED acting workspace; resets the user's password to
  `WEB_PASSWORD` so the real cookie sign-in authenticates.
- `the "<ws>" workspace has an admin credential "<label>"` — seeds a workspace-scoped
  `machine_tokens` row addressed by label (the foreign-credential target for the
  admin-cross scenario).
- When: `the member opens the "<ws>" project "<p>" board on the web` (+ `… by its
  real address` foreign variant + `… that never existed`); `the member opens issue
  <KEY> in the "<ws>" project "<p>" on the web` (+ `… that never existed`); `the
  member files issue "<title>" in the "<ws>" project "<p>" on the web` (+ `… in a
  project that never existed`); `the "<ws-A>" admin tries to revoke the "<ws-B>"
  credential "<label>" on the web`; `the member switches their active workspace to
  "<ws>"`.
- Then: `only "<ws>" data appears on the web` / `no "<ws>" data appears on the web` /
  `the new issue appears only in "<ws>" on the web` / `the member now sees only
  "<ws>" data on the web` / `the two web responses are refused identically` / `the
  web request is refused identically to a never-existed credential` / `nothing on
  the web reveals the "<ws>" board exists` / `… issue exists` / `no "<ws>" membership
  or credential is changed`.
- World fields added to `src/world.rs`: `mwt2_web_email`, `mwt2_web_password`,
  `mwt2_acting_ws`, `mwt2_last_body`, `mwt2_last_status`, `mwt2_first_refusal_body`,
  `mwt2_first_refusal_status`, `mwt2_credential_jti_by_label`,
  `mwt2_credential_revoked_before`.

**REUSED (verbatim machinery, not re-implemented):**
- The Background workspace/member/issue SEEDS — slice-1's registered step text
  (`workspace "…" exists with admin "…"`, `"…" has a member … with project … prefix
  …`, `the "…" project "…" has issues …-1 and …-2`). The two-workspace coexistence
  fixture is slice-1's; slice 2 reuses it and adds only the web surface.
- `InProcHarness::spawn` + per-scenario schema + the shared testcontainers PG16
  container (`support::harness`); the additive `ensure_harness` pattern (mirrors
  slice-1 — never resets after the first spawn, so the Background's two workspaces
  survive).
- The real cookie sign-in + CSRF dance (`sign_in_cookie` mirrors
  `feature_b_web_tier::sign_in_and_capture_cookie`); the shipped `signed_in_post`
  CSRF helper (`support::harness`) for every web POST.
- `Store::insert_machine_token`, `foundry_auth::hash_password`.

Deliberately NOT reused: a fresh harness reset on subsequent steps (would discard
the first workspace — same hazard slice 1 documents).

## Scaffold inventory (Mandate 7 / RED-ready)

- `.feature`: `crates/foundry-acceptance/tests/features/us-mwt-slice-02-web-boundary.feature`
  (9 scenarios; #1 active `@walking_skeleton`, #2-9 `@pending` per one-at-a-time).
- Steps: `crates/foundry-acceptance/src/steps/feature_mwt_slice_02_web_boundary.rs`
  (registered in `src/lib.rs` `pub mod steps {…}` + force-linked via
  `use … as _feature_mwt_s02` in `tests/acceptance.rs`).
- World fields added to `src/world.rs` (9 `mwt2_*` slots).
- One-line edit to slice-1 steps: `workspace_has_member_team_project` made `pub`
  (the documented slice-1→slice-2 reuse contract; the `#[given]` registration is
  unaffected, so slice-1 behaviour is unchanged).
- **No production-source scaffold stub needed**: the production surface this slice
  requires (the `0002` migration dropping `uniq_one_workspace`; the ADR-005
  membership-resolution rewrite of `signin.rs:140`; the `ActingWorkspace` newtype;
  the `/workspace/switch` switcher route) is DELIVER-authored production code, not a
  Rust module that step-defs import — so there is no module to stub. The crate
  COMPILES clean (`cargo test -p foundry-acceptance --no-run` → Finished), so the
  test is RED-not-BROKEN by construction.

**Gates run this slice:**
- `cargo test -p foundry-acceptance --no-run` → Finished (compiles, RED-not-BROKEN).
- `cargo fmt --all -- --check` → clean (exit 0).
- `cargo clippy --all-targets --release -- -D warnings` → clean (exit 0, 0 warnings).

## Test placement + precedent

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/*.rs` — matches
EVERY prior feature (us-w05a, us-tma, us-mt0x, us-0x, us-mwt-slice-01). Feature
filename `us-mwt-slice-02-web-boundary.feature` + step module
`feature_mwt_slice_02_web_boundary.rs` mirror the slice-named slice-1 precedent.

## Pre-requisites DELIVER must build (to flip RED → GREEN)

1. **`0002_multi_workspace.sql`** (ADR-006): `DROP INDEX uniq_one_workspace;` so the
   Background's second `workspaces` row can exist. (Shared with slice 1 — if slice 1
   already shipped `0002`, this is satisfied.)
2. **ADR-005 membership-resolution at the session seam**: replace the
   `first_workspace()` call-site (`signin.rs:140`) with `memberships_for_user` →
   single-membership auto-resolve / multi-membership explicit pick, stamping the
   ACTIVE workspace into `SessionUser.workspace_id`. Under two coexisting workspaces,
   `first_workspace()` picks an ARBITRARY row, so a member of Acme is not reliably
   scoped to Acme until this lands — the RED edge for scenarios 1-3, 8.
3. **`ActingWorkspace(Uuid)` newtype + the NEW check-arch LAYER-1e tenant-scoping
   rule** (ADR-002): web handlers consume the resolved `ActingWorkspace` (from the
   session), never a path-parsed id; the guard rule lands WITH this slice (its gold
   test is Slice 3+ per ADR-002). The web read/write scoping (`find_team_by_slug(
   workspace_id, …)`, `projects.rs`/`issues.rs`) is already shipped; this slice
   proves it under two real tenants.
4. **`POST /workspace/switch` switcher route** (ADR-005): verifies membership then
   re-stamps `SessionUser.workspace_id`. Absent today → scenario 9's switch POST
   404s, reding the post-switch read for the right reason.
5. The shipped `/admin/tokens/{jti}/revoke` web handler's non-enumerable 404 for a
   foreign jti (`admin_tokens.rs:188`) — already collapses missing/foreign to 404;
   this slice proves it when "foreign" means a coexisting second workspace.

## Fail-for-the-right-reason expectation (per scenario)

The crate compiles clean (no import/collection error → not BROKEN). At runtime,
against the real testcontainers PG16, every scenario reds for **MISSING_FUNCTIONALITY**:

| # | RED cause (the genuine missing functionality) |
|---|---|
| 1 | Background's 2nd `INSERT INTO workspaces` fails on `uniq_one_workspace` (until `0002`); once 2 coexist, `first_workspace()` resolves the session to an arbitrary workspace, so "only Acme data appears" fails until ADR-005 membership resolution is wired. |
| 2 | Same — issue-detail read scoped to the arbitrarily-resolved workspace until resolution is wired. |
| 3 | Same Background red; once 2 coexist, the file-issue write lands in the arbitrarily-resolved workspace, so the workspace-scoped row-count assertion fails until resolution is wired. |
| 4 | Background red; once 2 coexist + resolution wired, the foreign board reach must 404 identically to a never-existed board — fails until the acting-workspace scoping refuses the foreign team slug non-enumerably (shipped `find_team_by_slug(ws,…)` proves it once the acting ws is correctly Acme). |
| 5 | Same as 4 for issue detail. |
| 6 | Same as 4 for the file-issue write into a foreign project (must 404 + create no Globex row). |
| 7 | Background red; once 2 coexist + resolution wired, an Acme admin's revoke of a Globex jti must be a non-enumerable 404 with the Globex credential unchanged — fails until the acting-workspace `is_workspace_admin`/`revoke_token` path refuses the foreign jti as `NotFound`. |
| 8 | Background red; once 2 coexist, the contractor's session resolves via `first_workspace()` (arbitrary), so "acts on exactly Acme" fails until membership resolution picks the intended active workspace. |
| 9 | The `/workspace/switch` route does not exist yet → the switch POST 404s; even stubbed, the post-switch read fails until the switcher re-stamps the session's active workspace (ADR-005). |

This is the RED-phase entry signal DELIVER reads at PREPARE (ADR-025 D2). No scenario
reds for a fixture/import/setup reason — the genuine missing functionality is the
`0002` guard drop (shared with slice 1) + the ADR-005 web-leg resolution + switcher.

## Scope confirmation

**SLICE 2 ONLY.** Web htmx tier isolation (read/write/admin/refusal) + the
multi-membership active-workspace/switch at the WEB level. The JSON `/api/v1` +
machine-token + sign-in resolution surfaces (Slice 3), the uniform non-enumerability
matrix across ALL surfaces + the full adversarial timing/shape matrix (Slice 4),
migration-as-guarantee (Slice 5), and provisioning (Slice 6) are explicitly OUT — not
authored here.

## Upstream issues

**None.** No contradiction or production gap found. The web read/write scoping chain
(`find_team_by_slug(workspace_id, …)` → `is_team_member` → `find_project_by_slug`) is
already workspace-scoped on every slice-2 path, so there is no un-scoped web query
that could not be fed the acting workspace. (The two un-scoped single-row reads DESIGN
flagged in `upstream-changes.md` Finding 2 — `invites WHERE id`, `teams WHERE id` — are
NOT on any slice-2 web read/write path; they are folded into Slice 2/6 as DELIVER
hardening per slice-01's reconciliation, and do not block any slice-2 scenario.) No
`distill/slice-02-upstream-issues.md` is written.
