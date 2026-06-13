# Evolution — web-provisioning-flow (the deferred `/admin/instance/…` web provisioning surface)

**Finalized**: 2026-06-13
**DELIVER commits**: `02029e7` (01-01) → `0c32abd` (03-03) — the 11 DES-monitored TDD steps committed directly to `main` (trunk-based, no PRs) — plus the mid-feature regression fix `9efd8e9` (api route-absent 404 catch-all) and the review-remediation `ea09033` (findings D1–D3).
**Wave coverage**: requirements INHERITED (no DISCUSS re-run); DESIGN ratified here (D1–D6, 5 ADRs); DISTILL authored the web-provisioning scenarios; DELIVER shipped 11 steps across 3 phases. Legacy per-feature layout (`docs/feature/web-provisioning-flow/`).
**Scope**: this finalizes the **deferred web provisioning surface** of the shipped CLI-first `multi-workspace-provisioning` feature (whose ADR-002/D2 deferred the web flow to here; the original `multi-workspace-tenancy` ADR-004 sketched it as option (d)). The feature directory is PRESERVED (same policy as the parents).

## Feature summary

The parent `multi-workspace-provisioning` made tenants provisionable — but CLI-only (`foundry doctor provision-workspace`). A non-shell super-admin had no browser path to provision a workspace or grant instance authority. This feature ships that browser path.

It is overwhelmingly **EXTEND, not CREATE NEW** (DESIGN reuse verdict: **11 reuse/extend · 1 retire · 1 create-new**). It is a thin htmx **driving adapter** over the SHIPPED `provision_workspace` use-case + the SHIPPED `is_instance_admin` authz seam: the entire provisioning backend, the authz gate, and the atomic seed transaction shipped (and were mutation-hardened, including the gate-inversion mutant) with the parent. **ZERO new crate. ZERO migration** — `instance_admins` (`0011`) already shipped. The only genuinely new artifact is one focused module (`crates/foundry-app/src/instance_admin.rs`) + its templates; the only retirement is the legacy identity-blind `POST /workspaces` 409 route.

## What shipped

- **`GET /admin/instance/workspaces`** — a full HTML dashboard (no-JS entry point): a workspace list plus a provision form (name + first-admin email) and a grant-super-admin form (email), each carrying a double-submit `_csrf` field.
- **`POST /admin/instance/workspaces`** — provision: builds a `ProvisionRequest` from session + form, calls the shipped `Services::provision_workspace`, returns an htmx success fragment reporting the new workspace id + the signed first-admin invite link (informational per D5 — see deferred).
- **`POST /admin/instance/super-admins`** — grant: resolves email → `grant_instance_admin` (idempotent; granting twice records the role exactly once). Non-committal for unknown emails (no user-enumeration oracle).
- **Security envelope**: all three routes mount UNDER the shipped session + double-submit CSRF layers, gated by a fail-closed inline `require_instance_admin` check. Refusal is a **uniform, non-enumerable 404** (`resource_not_found_page()`) — **byte-identical for signed-out AND signed-in-non-admin** callers, byte-identical to a never-existed path. No 403/401/redirect oracle; no 403-vs-404 oracle. A revert-reds-it litmus binds the byte-identity assertion (collapsing the two refusal arms re-REDs it).
- **`list_workspaces`** — one thin, non-tenant-scoped instance read for the dashboard list (the single new store query; `instance_admin` added to the LAYER-1e allow-list, D6).
- **Provisioned-tenant isolation proven**: a browser-provisioned workspace is a real isolated tenant — its first admin sees only that workspace's data through the shipped `resolve_active_workspace` membership seam; existing workspaces are left byte-for-byte untouched by a provision.

## Decisions realized (D1–D6)

| # | Decision | Status |
|---|---|---|
| **D1** | Routes/screens = ONE page `GET /admin/instance/workspaces` (list + provision form + grant form) + `POST …/workspaces` (provision) + `POST /admin/instance/super-admins` (grant); htmx fragments for POSTs, full page for GET. | **IMPLEMENTED** |
| **D2** | Inline `require_instance_admin` gate (read `SessionUser` → `is_instance_admin` → uniform 404 on signed-out OR non-admin); grant non-committal for unknown emails. NO new middleware tier. | **IMPLEMENTED** |
| **D3** | RETIRE the legacy `POST /workspaces` 409 route — DELETED outright (not left inert), per the 2026-06-13 AGENTS.md "## Dead code" policy. The gated admin POST is the sole web provisioning path. | **IMPLEMENTED** (retired) |
| **D4** | Thin driving adapter over the SHIPPED `provision_workspace` / `grant_instance_admin`; one thin non-tenant-scoped `list_workspaces` read; NO new domain/store logic, NO migration. | **IMPLEMENTED** |
| **D5** | Invite-accept (`/invites/accept` + password-set) is OUT of v1; the first-admin invite link is informational. | **IMPLEMENTED** (accept flow deferred) |
| **D6** | Add `instance_admin` to the LAYER-1e `is_tenant_scoping_allowlisted` (one line) — the line the parent ADR-003 foresaw a future web file would owe. | **IMPLEMENTED** |

D5 keeps the credential-establishment path out (not stubbed) — provisioned-admin sign-in is proven via the shipped `resolve_active_workspace` membership seam, NOT a real `/invites/accept` route (which does not exist on either CLI or web).

## How it was built (DELIVER) — the 11-step TDD arc

**11 DES-monitored TDD steps across 3 phases**, each driven by `@real-io` cucumber scenarios over the real surfaces (real axum router, real session + CSRF layers, real testcontainers PG16), every step running all 5 DES phases (integrity exit 0).

| Phase | Steps | What it proved |
|---|---|---|
| **01 — web provisioning surface + happy paths (D1/D4/D6)** | 01-01..04 | walking skeleton: super-admin provisions a real isolated workspace from the browser end-to-end through session + CSRF to the shipped use-case; dashboard lists workspaces + offers both forms; super-admin grants super-admin from the browser; granting twice is idempotent |
| **02 — web authz + non-enumerability (D2, security)** | 02-01..04 | grant form is not a user-enumeration oracle; signed-out request refused like a never-existed path; signed-in non-super-admin refused byte-identically to signed-out (revert-reds-it litmus); provision without a valid CSRF token is refused by the shipped middleware (no workspace created) |
| **03 — legacy-route retirement + provisioned-tenant isolation (D3/D4)** | 03-01..03 | the legacy `POST /workspaces` route no longer exists (404, not the old 409); a browser provision leaves existing workspaces byte-for-byte untouched; the browser-provisioned workspace is a real isolated tenant through the `resolve_active_workspace` seam |

### A regression was caught and fixed mid-feature

Step 02-02's CSRF-wrapped router fallback made unrouted `/api/v1` POSTs return a CSRF-403 instead of the route-absent 404 — breaking the shipped slice-06 "provisioning unreachable from the bearer API" security scenario (the `api≠mint` boundary). Fixed in `9efd8e9` with a **CSRF-exempt route-absent 404 catch-all on `/api/v1`** in `foundry-api`, so per-surface fallbacks now hold independently: web = CSRF-wrapped, api = route-absent 404. The full acceptance lane returned to green.

## Quality at ship

`cargo xtask ci` — **ALL GATES GREEN**:

- **fmt**: `cargo fmt --all --check` clean.
- **clippy**: `cargo clippy --all-targets --release -- -D warnings` clean.
- **`cargo xtask check-arch`**: PASSED (the one new LAYER-1e allow-list line, D6).
- **`@all` acceptance**: **285 scenarios / 2348 steps** green (parent suites plus this feature's new web-provisioning scenarios; green-before stays green-after).
- **Adversarial review**: **APPROVED** — Testing Theater: **none found** (the per-step falsifiability litmus proofs were verified to bind to production code, not to fixtures).
- **Review findings D1–D3**: ALREADY FIXED in `ea09033` (fail-closed authz-probe logging, `_csrf` visibility, dedup test constant) — done, not outstanding.
- **Zero new crates. Zero migration.**

One infra note: a Docker crash-loop (host k3s resource starvation) blocked step 03-02 mid-cycle; it was retried clean once Docker recovered — not a feature defect.

## Deferred / follow-ups

**The gating gap (highest-value next feature):**
- **`/invites/accept` route** (token verify + password-set form + consume-invite transaction). A provisioned first-admin still cannot truly sign in on EITHER the CLI or the web — the emitted invite link is informational/dead on both surfaces (D5). This is the single highest-value next feature; building it fixes both surfaces' dead link at once.

**Mutation testing — deferred for this feature (user-ratified 2026-06-13):**
- The new code is acceptance-only-covered, and `cargo-mutants` would run the full Docker lane per mutant (hours under k3s load). It is thin adapters over already-100%-mutation-hardened shipped code (the provisioning gate-inversion mutant was already killed in the parent), with per-step falsifiability proofs + a no-Testing-Theater review verdict as the effectiveness evidence.
- **Recommendation**: a nightly/follow-up scoped mutation pass on `instance_admin.rs` + `list_workspaces` + the new `/api/v1` route-absent catch-all.

**Carried from prior features:**
- Prometheus exporter for `foundry_token_mutations_total`.
- Per-workspace backup/restore (OD-5) — whole-instance backup unchanged.
- Key-rotation UX.

## Pointers

- Spec (preserved): `docs/feature/web-provisioning-flow/{design,distill,deliver}/` — notably `design/wave-decisions.md` (D1–D6 ratification), the 5 ADRs (`adr-001..005`), `design/upstream-changes.md` (the dead `/invites/accept` link finding), and the DISTILL scenarios.
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/web-provisioning-flow/deliver/roadmap.json` (3 phases / 11 steps) + `execution-log.json` (DES-verify-integrity clean) + `.develop-progress.json`.
- Core production files:
  - Web adapter (NEW): `crates/foundry-app/src/instance_admin.rs` (`require_instance_admin` gate + the three handlers) + its Askama templates.
  - Route registration + legacy retirement: `crates/foundry-app/src/lib.rs` (`build_router` — three new routes; legacy `POST /workspaces` removed), `crates/foundry-app/src/bootstrap.rs` (the retired `create_workspace` 409 handler).
  - Dashboard read: `crates/foundry-store/src/lib.rs` (`list_workspaces`).
  - api boundary fix: the `/api/v1` route-absent 404 catch-all in `foundry-api` (`9efd8e9`).
  - LAYER-1e allow-list: `xtask/src/check_arch.rs` (`instance_admin` added to `is_tenant_scoping_allowlisted`, D6).
  - Reused verbatim (shipped, hardened): `crates/foundry-services/src/lib.rs` (`provision_workspace`), `crates/foundry-store/src/lib.rs` (`is_instance_admin`, `grant_instance_admin`, `user_id_by_email`, the provision tx).
  - Acceptance: `crates/foundry-acceptance/tests/features/us-mwt-web-provisioning.feature` + step defs.
- Predecessors: `docs/evolution/2026-06-12-multi-workspace-provisioning.md` (the CLI-first surface this completes) and `docs/evolution/2026-06-11-multi-workspace-tenancy.md` (the isolation core).
