# Evolution — multi-workspace-tenancy (milestone: isolation core, slices 1-4)

**Finalized**: 2026-06-11
**Ship commit**: `45ff99c` (tip; 18 DES steps across 4 phases) off `8a3624b` (DISCUSS) — feature range `b6442be..45ff99c`.
**Wave coverage**: full nWave pipeline — DISCUSS → DESIGN → DISTILL → DELIVER, delivered **slice-by-slice with a user checkpoint between slices** (legacy per-feature layout; trunk-based, committed directly to `main`).
**Milestone scope**: this finalizes the **isolation core — slices 1-4 only**. Slices 5-6 are DEFERRED to a follow-up feature (`multi-workspace-provisioning`); see "Deferred" below. The entire feature directory is PRESERVED (the slice-05/06 briefs + DISCUSS/DESIGN docs are the seed for that follow-up).

## Feature summary

Foundry was **single-workspace** — `CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true))` (`0001_init.sql:15`) forbade a second row, and `bootstrap.rs` `create_workspace` hard-409'd a second one. This milestone makes tenancy **REAL**: multiple workspaces coexist in one instance with **genuine per-tenant data isolation across ALL surfaces** — the web htmx tier, the JSON `/api/v1`, machine-token auth, and sign-in/sessions. The schema was already multi-tenant-*shaped* (every tenant table FKs to `workspaces(id)`, every read binds `workspace_id`, the non-enumerable lookup idiom shipped in `attachments.rs`), so the work is overwhelmingly **EXTEND, not CREATE NEW** — it removes the guard, generalizes the request→workspace resolution seam from "the sole workspace" to "the membership-resolved active workspace", makes that seam un-forgettable at build time, and proves the boundary end-to-end with REAL two-workspace (Acme / Globex) fixtures. **ZERO new crates.**

This was the documented blocker that **UNBLOCKED two accepted residuals** carried by `machine-token-admin-ux` / `token-management-api`: the synthetic-uuid cross-workspace tests (real two-workspace fixtures — **CLOSED here**, slice 3) and the per-principal rate-bucket map eviction (residual F2 — **deferred to slice 6**).

Ratified DISCUSS decisions designed-to:
- **OD-1 — shared-schema with a `workspace_id` discriminator** (the model the schema already used; lowest risk).
- **OD-2 — multi-membership**: one user/email MAY belong to many workspaces (`users` is global, membership is the M:N `workspace_memberships`); implies a workspace-selection / switcher UX (shipped here).
- **OD-3 — instance super-admin** for provisioning authority. **The role itself is slice 6 / DEFERRED** — the provisioning surface is NOT in this milestone.

## What shipped (slices 1-4, security-bearing)

- **The migrations (slice 1)**: `0009_multi_workspace.sql` **drops `uniq_one_workspace`** (a second workspace row can now exist) and removes the application 409 guard; `0010_active_workspace.sql` adds the per-session **active-workspace** column. Forward-only; no existing row's data is touched.
- **The request→workspace RESOLUTION seam (slice 2/3)** — the central mechanism (ADR-001):
  - **Web/session**: the active workspace is resolved by membership via `Store::resolve_active_workspace` (deterministic `w.id` tiebreak), **replacing the buggy `first_workspace()`** that returned an unordered arbitrary workspace. `SessionUser{ user_id, workspace_id }` now carries the *active* workspace, not "the sole one".
  - **API**: `token.workspace_id` (the SHIPPED `MachinePrincipal` / `Principal::Machine{ workspace_id }`) IS the authoritative acting workspace for `/api/v1` — the seam was already shipped; the milestone proves it refuses cross-tenant calls.
  - **Fail-closed**: a credential whose holder resolves to NO workspace membership is refused, not silently defaulted.
- **The `ActingWorkspace` newtype + the NEW check-arch LAYER-1e tenant-scoping guard (slice 2)** (ADR-002): a one-field newtype the handlers consume INSTEAD of a client-supplied id, making "handler trusts the resolved seam" the only typed path (NFR-MWT-SEC-06). The new `check_app_tenant_scoping` AST rule (`xtask/src/check_arch.rs:314`) **fails the build** if a `foundry-app` handler scopes a tenant query by a request-parsed workspace id instead of the resolved `ActingWorkspace`/`user.workspace_id` — the "forgot to scope / trusted the client" footgun becomes a compile-gate failure, proven by a **planted-violation gold test**.
- **Uniform non-enumerable 404 across EVERY surface (slice 2/4)** (ADR-003): a request for a foreign-workspace resource is refused **identically** to a request for a never-existed one — web htmx tier → the shipped `resource_not_found_page()`; JSON `/api/v1` → the shipped `status_for` 404 envelope. No 403-vs-404 oracle, no id/slug echo, no body-shape diff.
- **The multi-membership `/workspace/switch` switcher (slice 2)** (ADR-005): single-membership auto-resolves with no prompt; multi-membership picks at sign-in and a `POST /workspace/switch` re-stamps the session's active workspace. **Membership-guarded and fail-closed** — `set_active_workspace` refuses to point a session at a workspace the user is not a member of (`session.rs:96`).
- **The token-API synthetic-uuid residual CLOSED (slice 3)** (NFR-MWT-TEST-01 / DM8): the cross-workspace token list/revoke confinement tests now run on **real two-workspace fixtures** (a real Globex token is excluded from Acme's list and is non-enumerably refused on revoke), not synthetic uuids.

## The security headline — slice 4's adversarial non-enumerability matrix found + closed 4 real enumeration oracles

The per-surface slices (1-3) were green by inheritance from the shipped `workspace_id` scoping. Slice 4 ran a **comprehensive adversarial non-enumerability matrix** (foreign-id ≡ missing-id, every surface) and found **4 REAL enumeration oracles** those slices had missed (ref `distill/slice-04-upstream-issues.md`, ISSUE-04-01 / ISSUE-04-02):

| # | Surface (web) | Oracle |
|---|---|---|
| 1 | attachment **download** (`attachments.rs::download_attachment`) | foreign reach rendered the slug-echoing `team_not_found_page` (`No team with slug "platform" exists…`); never-existed rendered `not_found_page` echoing the attachment UUID — a body-shape diff PLUS a foreign-id echo |
| 2 | **comment** (`comments.rs::submit_comment`) | cross-tenant write died at the team layer → slug-echoing `team_not_found_page`; never-existed → `issue_not_found_page` — shape diff + slug echo |
| 3 | **state-change** (`issues.rs::submit_state_change`) | same family → `team_not_found_page` vs `project_not_found_page` — shape diff + slug echo |
| 4 | attachment **upload** (`attachments.rs::submit_upload`) | same family; ALSO unmasked a CSRF step-def bug (multipart needs the `x-csrf-token` HEADER, not the `_csrf` form field) that had been 403'ing the foreign upload before it reached the real refusal surface |

Each of the four was a slug-echoing "Team not found" body distinguishable from a never-existed reach. All collapsed to the single uniform `resource_not_found_page()`; the now-dead slug-echoing helpers were removed. The 04-05 cross-surface **oracle-hunt capstone** confirmed **no cross-tenant 403 anywhere** (a cross-tenant reach 404s at the team layer above the membership 403 branch and never reaches it — the ADR-003 boundary clause). The intra-workspace membership 403 (`non_member_page`) is retained, correctly, as a non-cross-tenant concern.

## How it was built (DELIVER)

**18 DES-monitored TDD steps across 4 phases** (90 PREPARE/RED/GREEN/COMMIT phase events), each driven by `@real-io` cucumber scenarios over the real surfaces (real HTTP, real EdDSA bearer, real testcontainers PG16), delivered slice-by-slice with a user checkpoint between slices.

| Slice / Phase | Steps | What it proved |
|---|---|---|
| **1 — walking-skeleton coexistence** | 01-01..03 | `0009` drops `uniq_one_workspace`; two real workspaces with disjoint A/B sets coexist; fail-closed when a credential resolves to no workspace |
| **2 — web boundary (the seam/guard/switcher)** | 02-01..05 | session active-workspace resolution (ADR-005) replacing `first_workspace()`; web read+write isolation; `ActingWorkspace` + the LAYER-1e guard; admin authority does not cross tenants; the `/workspace/switch` switcher |
| **3 — API + auth boundary + residual closure** | 03-01..05 | an Acme-bound token's write lands only in Acme; cross-tenant API reach → uniform 404; token list/revoke confinement on REAL fixtures (residual CLOSED); the session-resolution contract (single/multi/none); verify-path-unchanged regression |
| **4 — non-enumerability matrix** | 04-01..05 | the adversarial matrix above — 4 oracles found + closed; the cross-surface oracle-hunt capstone (no cross-tenant 403 anywhere) |

A rigorous **revert-reds-it litmus** was applied to every green-by-inheritance scenario to prove it was real (not fixture theater): reverting the production change had to re-RED the scenario on the exact security assertion (e.g. the body-equality assertion for the non-enumerability cells). Several **latent seed bugs** were found + fixed along the way — the unordered `first_workspace()` resolution, the four slug-echo oracles, and the multipart-CSRF masking step-def bug.

## Quality at ship

`cargo xtask ci` — **ALL GATES GREEN**:

- **fmt**: `cargo fmt --all --check` clean.
- **clippy**: `cargo clippy --all-targets --release -- -D warnings` clean.
- **check-arch**: green — now includes the **new LAYER-1e tenant-scoping rule** alongside `api≠HTML` / `api≠ad-hoc-authz` / `api≠mint` / JWT-alg-pin / dependency-direction (and the planted-violation gold test proves the guard bites).
- **release build** + **cargo-deny**: clean.
- **`@all` acceptance**: **273 scenarios / 2279 steps** green — the full multi-workspace suite plus the entire prior suite (green-before stays green-after).
- **Zero new crates.**

## Deferred to the follow-up feature (`multi-workspace-provisioning`)

These are **NOT shipped in this milestone**. Their DISCUSS/DESIGN docs + slice briefs are preserved in `docs/feature/multi-workspace-tenancy/` as the seed:

- **Slice 5 — existing-install migration GUARANTEE** (`slices/slice-05-existing-install-migration.md`, US-MWT06, ADR-006): the formal, user-visible upgrade-safety proof that a real pre-feature single-workspace install upgrades **forward-only, no-loss**, the existing workspace becomes **workspace 1**, and existing sessions/tokens keep resolving. NOTE: the `0009` migration **already drops the guard** — slice 5 is the formal before/after row-equality + auth-suite-still-green proof against a real pre-feature DB snapshot, not the schema change.
- **Slice 6 — provision-and-prove** (`slices/slice-06-provision-and-prove.md`, US-MWT07/08, ADR-004): the **instance super-admin PROVISIONING surface** (create a tenant + seed its first admin via the bootstrap/invite idiom; refuse non-super-admins) AND closing the **rate-bucket-eviction residual** (LRU/idle eviction on the per-principal map so it stays bounded under many tenants).

The **6 ADRs** (`design/adr-001..006`) are the design seed: 001/002/003/005 are IMPLEMENTED by this milestone; 004 (super-admin role + provisioning surface) and the migration-guarantee half of 006 are DEFERRED.

## Residuals / follow-ups

- **Rate-bucket map eviction** (residual F2) — still OPEN; tracked to slice 6. Bounded today only by the active-principal count.
- **Existing-install migration guarantee** — slice 5 (the `0009` drop is shipped; the formal upgrade-safety proof is deferred).
- **Per-workspace backup/restore** (OD-5) — deferred as a follow-up; whole-instance backup unchanged for v1.
- The security **CORE — tenant isolation + cross-tenant non-enumerability — IS complete and provable** across every surface in this milestone.

## Pointers

- Spec (preserved): `docs/feature/multi-workspace-tenancy/{discuss,design,distill,slices}/` — notably `discuss/wave-decisions.md` (OD-1/2/3 ratification), the 6 ADRs, the 4 slice distill docs, and `distill/slice-04-upstream-issues.md` (the oracle findings).
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/multi-workspace-tenancy/deliver/roadmap.json` (4 phases / 18 steps) + `execution-log.json` (90 phase events; `des-verify-integrity` clean — all 18 steps DONE with complete DES traces).
- Core production files:
  - Migrations: `crates/foundry-store/migrations/0009_multi_workspace.sql`, `0010_active_workspace.sql`.
  - Resolution seam: `crates/foundry-store/src/lib.rs` (`resolve_active_workspace`, `set_active_workspace`).
  - Web tier: `crates/foundry-app/src/session.rs` (`ActingWorkspace` newtype + the `/workspace/switch` switcher), `rate_limit.rs`, `projects.rs`, `comments.rs`, `issues.rs`, `attachments.rs` (the uniform-404 collapse).
  - Build-time guard: `xtask/src/check_arch.rs` (the LAYER-1e `check_app_tenant_scoping` rule + planted-violation gold test).
  - Acceptance: the slice `.feature` files + step defs under `crates/foundry-acceptance/`.
- Predecessors this unblocked: `docs/evolution/2026-06-07-machine-token-admin-ux.md`, `docs/evolution/2026-06-08-token-management-api.md` (the synthetic-uuid + rate-bucket residuals).
