# DESIGN Decisions — web-provisioning-flow

> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode.
> The **deferred web provisioning surface** of the shipped `multi-workspace-provisioning` feature
> (its ADR-002 D2 deferred the web flow here; the parent `multi-workspace-tenancy` ADR-004 sketched
> it as option (d)). Requirements INHERITED (no DISCUSS re-run); seed = the parent provisioning
> feature's ADRs + the original tenancy ADR-004 + the shipped code seams.
> Legacy per-feature layout (`docs/feature/web-provisioning-flow/design/`). Trunk-based.

## Reading checklist
- ✓ `multi-workspace-provisioning/design/adr-002-provisioning-surface.md` (D2 — CLI-first; **web DEFERRED to HERE**)
- ✓ `multi-workspace-provisioning/design/adr-001-first-superadmin-bootstrap.md` (super-admin identity)
- ✓ `multi-workspace-provisioning/design/adr-003-instance-admin-role-schema.md` (instance_admins / is_instance_admin; **the "future web file owes one allow-list line" note**)
- ✓ `multi-workspace-provisioning/design/adr-004-migration-guarantee-approach.md`
- ✓ `multi-workspace-provisioning/design/architecture.md` (the shipped provisioning path)
- ✓ `multi-workspace-provisioning/design/upstream-changes.md` (CLI-first revision + the bootstrap.rs:301 409 guard STILL PRESENT)
- ✓ `multi-workspace-tenancy/design/adr-004-instance-super-admin-role.md` (the ORIGINAL web-first design — option (d), /admin/instance/workspaces, session+CSRF)
- ✓ `multi-workspace-tenancy/design/architecture.md` (resolution seam, ActingWorkspace, LAYER-1e, the slice-2 web htmx tier)
- ✓ `docs/evolution/2026-06-12-multi-workspace-provisioning.md` (what shipped; the deferred web-flow + invite-accept follow-ups)
- ✓ `crates/foundry-services/src/lib.rs:227-270` (`provision_workspace` use-case, authz-gated → ServiceError::Forbidden — REUSE)
- ✓ `crates/foundry-store/src/lib.rs:1162-1259` (`is_instance_admin`, `grant_instance_admin`, `user_id_by_email`, `provision_workspace` tx — REUSE)
- ✓ `crates/foundry-app/src/admin_cli.rs:395-671` (the CLI `provision-workspace` + `grant-super-admin` — the use-case call shape to mirror)
- ✓ `crates/foundry-app/src/bootstrap.rs:301-333` (the legacy `POST /workspaces` 409 guard — the retire/supersede point)
- ✓ `crates/foundry-app/src/session.rs:64-191` (`SessionUser`, `SESSION_KEY_USER_ID`, the `/workspace/switch` membership-guard + non-enumerable 404)
- ✓ `crates/foundry-app/src/csrf.rs:96-173` (the double-submit CSRF middleware the new POSTs mount under)
- ✓ `crates/foundry-app/src/admin_tokens.rs` (the closest precedent: session-gated, CSRF-protected, htmx admin route group)
- ✓ `crates/foundry-app/src/lib.rs:234-388` (`build_router` — where the three new routes register; confirmed NO `/invites/accept` route exists)
- ✓ `xtask/src/check_arch.rs:387-396` (LAYER-1e `is_tenant_scoping_allowlisted` — the one-line addition)

## Key Decisions (DDD-numbered)

| # | Decision | Rationale | ADR |
|---|---|---|---|
| **D1** | Routes/screens = ONE page `GET /admin/instance/workspaces` (list + provision form + grant form) + `POST …/workspaces` (provision) + `POST /admin/instance/super-admins` (grant). htmx fragment responses for POSTs, full page for GET. | Minimal-but-coherent v1; mirrors the shipped `/admin/tokens` admin idiom (session-gated, CSRF, htmx). Three handlers in one file is the smallest surface that lets a non-shell super-admin do the two shipped operations (provision, grant). | adr-001 |
| **D2** | Web authz = an **inline `require_instance_admin` gate** (read `SessionUser` → `store.is_instance_admin` → uniform 404 via `resource_not_found_page()` on signed-out OR non-admin). NO new middleware tier. The grant action returns a non-committal result for unknown emails. | Mirrors the shipped `/workspace/switch` fail-closed + non-enumerable idiom (G4) exactly; the codebase gates inline, not via a layer (G3). **Non-enumerable** is the security crux: an unauthorized user must not learn the surface exists — no 403-vs-404 oracle, consistent with the shipped tenancy boundary. | adr-002 |
| **D3** | The `bootstrap.rs:301` 409 guard = **RETIRE the legacy `POST /workspaces` route**; the new `/admin/instance/workspaces` POST is the sole web provisioning path. (Alternative kept on record: leave it inert as defence-in-depth.) | The legacy handler only ever hard-409s (it predates multi-workspace); the real, gated creation now lives in the new handler. Retiring removes a dead, identity-blind route rather than leaving a confusing second "create workspace" POST. REALISES the parent upstream Finding 2's "replace point". | adr-003 |
| **D4** | Reuse vs new = the web layer is a **thin driving adapter** over the SHIPPED `Services::provision_workspace` / `grant_instance_admin`. NO new domain/store logic, NO migration. At most one thin non-tenant-scoped workspace-list read for the dashboard. | The entire provisioning backend, authz gate, and atomic seed tx shipped and are mutation-hardened (incl. the gate-inversion mutant). This feature is a NEW DRIVING ADAPTER, not new domain logic — the framing is explicit in the task and confirmed by the code (G1/G2). | adr-004 |
| **D5** | First-admin onboarding / invite-accept = **OUT of this v1 (a further follow-up).** The success fragment shows the same signed `/invites/accept?…` link the CLI emits (informational). | **G7**: there is NO `/invites/accept` route, no `consume_invite` store fn, no password-set handler — the emitted link is a dead URL today (the same approximation the parent slices used). A real accept vertical (route + token verify + password-set form + consume-invite tx) is LARGER than this whole feature; bundling it would blow v1 scope. **Flagged for ratification.** | adr-005 |
| **D6** | LAYER-1e allow-list = **add `instance_admin` to `is_tenant_scoping_allowlisted`** (`check_arch.rs:394`) — one line. | The new handler file names a *literal new* workspace id (provisioning is non-tenant-scoped), so it must not trip the LAYER-1e detector. This is the EXACT line the parent ADR-003 recorded as owed by "a future web surface in a new file." Inherits D7 lineage. | adr-002/004 |

## Architecture Summary
- **Pattern**: modular monolith + ports-and-adapters (inherited, in force). The web surface is a
  NEW **driving adapter** (`foundry-app/src/instance_admin.rs`) over the SHIPPED
  `foundry-services` provisioning use-case; authz stays in services/store; the adapter only reads
  the session and maps use-case results to HTML.
- **Paradigm**: Rust (composition-over-inheritance, functional-core / imperative-shell —
  unchanged; established by the parent, NOT re-decided here).
- **Key components**: `instance_admin.rs` web handlers + `require_instance_admin` gate (NEW,
  adapter); 2-3 Askama templates (NEW); three `.route(...)` lines in `build_router` (EXTEND); one
  LAYER-1e allow-list line (EXTEND). Everything below the gate — `is_instance_admin`,
  `provision_workspace`, `grant_instance_admin`, the seed tx, CSRF/session — is SHIPPED and REUSED.

## Reuse Analysis (verdict: 11 REUSE/EXTEND · 1 RETIRE · 1 CREATE NEW — overwhelmingly REUSE)

| # | Existing component | File | Overlap | Decision | Justification |
|---|---|---|---|---|---|
| 1 | `Services::provision_workspace` (gated) | `foundry-services/src/lib.rs:227` | Provisioning use-case | **REUSE (verbatim)** | Shipped, gated, mutation-hardened. Adapter builds `ProvisionRequest` and calls it. |
| 2 | `grant_instance_admin` + `user_id_by_email` + `is_instance_admin` | `foundry-store/src/lib.rs:1162` | Grant + authz seam | **REUSE (verbatim)** | The CLI already drives the same pair; web does too. |
| 3 | `Store::provision_workspace` tx | `foundry-store/src/lib.rs:1212` | Atomic create+seed | **REUSE** | Driven only through #1. |
| 4 | `csrf_middleware` (double-submit) | `csrf.rs:96` | POST protection | **REUSE** | New routes mount under the shipped layer; forms carry `_csrf`. |
| 5 | Session extract idiom | `session.rs`, `bootstrap.rs:23` | Read signed-in user | **REUSE** | Same read every web handler uses. |
| 6 | `/workspace/switch` fail-closed + `resource_not_found_page()` | `session.rs:138`, `bootstrap.rs:382` | Non-enumerable refusal | **REUSE (shape)** | The gate copies the exact uniform-404 response. |
| 7 | `/admin/tokens` web admin idiom | `admin_tokens.rs` | Admin route group | **REUSE (shape)** | Closest precedent for a gated, CSRF, htmx admin page. |
| 8 | Askama `base.html` + page/fragment idiom | `templates/*`, `views.rs` | HTML rendering | **REUSE (shape)** | New templates follow `extends base.html` + `partials/`. |
| 9 | `InviteToken::new` + invite-url builder | `foundry-auth`, `bootstrap.rs:267`, `admin_cli.rs:499` | Signed invite link | **REUSE** | Success fragment shows the same link the CLI prints (dead in v1 per G7). |
| 10 | `build_router` registration | `lib.rs:234` | Mount routes | **EXTEND** | Add three `.route(...)` lines before the csrf+session layers. |
| 11 | `bootstrap::create_workspace` 409 (`POST /workspaces`) | `bootstrap.rs:301` | Legacy web POST | **RETIRE / SUPERSEDE (D3)** | Identity-blind hard-409; superseded by the gated `/admin/instance/workspaces` POST. |
| 12 | `instance_admin.rs` adapter + templates | — (does not exist) | The driving adapter | **CREATE NEW** | A driving adapter for a new surface; one focused module + 2-3 templates. |
| 13 | LAYER-1e allow-list entry | `check_arch.rs:387` | Tenant-guard exemption | **EXTEND (one line) (D6)** | Provisioning names a literal new workspace id; the parent ADR-003 foresaw this exact line. |

## Technology Stack
- **Rust** (inherited): axum, askama, tower_sessions, the shipped `csrf_middleware`, sqlx,
  `foundry-auth` (`InviteToken`). **ZERO new crates.**
- **PostgreSQL** (one instance, inherited): **NO migration** — `instance_admins` (`0011`) already
  shipped.
- **Enforcement**: `cargo xtask check-arch` (inherited; ONE new allow-list line, D6).
- **OSS-first / license**: all inherited deps; no proprietary; no new dependency to license.

## Constraints Established / honored
- ONE binary · ONE Postgres · NO Redis · NO Node · NO CDN · **ZERO new crates** · **ZERO migration**.
- The surface is super-admin-gated, fail-closed, **non-enumerable** (uniform 404, no 403 oracle),
  and OFF the `/api/v1` bearer surface (`api≠mint`).
- Provisioning POSTs are CSRF-protected by the SHIPPED double-submit layer (reused byte-for-byte).
- The `foundry-acceptance` suite green-before stays green-after.
- The new handler file owes exactly ONE LAYER-1e allow-list line (D6) — as the parent ADR-003 foresaw.

## Earned-Trust (probe-don't-assume) commitments for DISTILL/DELIVER
- **Non-enumerability PROBED**: a signed-in non-super-admin AND a signed-out user get byte-identical
  404s (status + body) on every `/admin/instance/…` route; revert-reds-it litmus (removing the gate
  must re-RED the assertion).
- **CSRF PROBED**: a provisioning POST with a missing/mismatched `_csrf` is refused (403) on the new
  route, exercising the shipped double-submit middleware.
- **Defence-in-depth PROBED**: with the adapter gate test-bypassed, `Services::provision_workspace`
  still refuses a non-super-admin (the shipped gate-inversion mutant guards this).
- **No new domain regression**: the existing 275-scenario acceptance suite stays green (the feature
  adds an adapter, not domain logic).

## Status — IMPLEMENTED / SHIPPED (finalized 2026-06-13)

**All decisions D1–D6 are IMPLEMENTED and shipped to `main`** via the 11 DES-monitored TDD steps
(`02029e7` → `0c32abd`) plus the regression fix `9efd8e9` and review-remediation `ea09033`.
`@all` acceptance 285/285 scenarios (2348/2348 steps) green; fmt + clippy `-D warnings` clean;
`cargo xtask check-arch` PASSED; adversarial review APPROVED (no Testing Theater). Zero new crate,
zero migration. See `docs/evolution/2026-06-13-web-provisioning-flow.md`.

| # | Decision | Status |
|---|---|---|
| **D1** | One-page routes/screens (GET dashboard + 2 POSTs, htmx). | **IMPLEMENTED** |
| **D2** | Inline `require_instance_admin` gate + uniform non-enumerable 404; non-committal grant. | **IMPLEMENTED** |
| **D3** | RETIRE legacy `POST /workspaces` 409 route (deleted outright). | **IMPLEMENTED** |
| **D4** | Thin driving adapter + `list_workspaces` read; no new domain logic, no migration. | **IMPLEMENTED** |
| **D5** | Invite-accept OUT of v1 (link informational). | **IMPLEMENTED** (accept flow deferred) |
| **D6** | +1 LAYER-1e allow-list line for `instance_admin`. | **IMPLEMENTED** |

## Open decisions — RATIFIED by user 2026-06-13 (before DISTILL)

Both scope-defining decisions are now CONFIRMED at their recommended options. D1/D2/D4/D6 stand at
their grounded defaults (no disagreement raised).

1. **[D5 / adr-005] Invite-accept (`/invites/accept` + password-set) — RATIFIED OUT of v1.** The
   first-admin invite link stays informational; the accept vertical (route + token verify +
   password-set form + consume-invite store tx) is a SEPARATE follow-up feature. DISTILL must NOT
   author scenarios that require a provisioned admin to actually sign in via the link. (See G7 /
   upstream-changes.md.)
2. **[D3 / adr-003] RETIRE the legacy `POST /workspaces` 409 route — RATIFIED RETIRE.** DELIVER
   DELETES the dead route outright (not left inert), per the repo `AGENTS.md` "## Dead code"
   policy added 2026-06-13 (pre-stable: remove superseded code rather than carry it inert). The
   `is_instance_admin`-gated `/admin/instance/workspaces` POST is the sole web provisioning path.

D1 (screen scope), D2 (inline non-enumerable gate), D4 (thin adapter), and D6 (allow-list line) sit
at grounded defaults dictated by the shipped seams — confirmed, no changes.

## Upstream Changes
See `upstream-changes.md` — one finding: the parent provisioning feature's evolution doc lists the
web flow as deferred but its emitted `/invites/accept` invite link has NO route behind it (a dead
URL today). This feature surfaces that the web success path (and the CLI's) point at a
not-yet-built accept flow; D5 keeps that accept vertical OUT of v1. Per the back-propagation
contract: original quoted, new assumption + rationale stated; the parent docs are NOT modified.
</content>
