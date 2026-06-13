# Test Scenarios — web-provisioning-flow (DISTILL)

> Quinn (nw-acceptance-designer), DISTILL wave. Framework: cucumber-rs.
> Feature SSOT: `crates/foundry-acceptance/tests/features/us-mwt-web-provisioning.feature`.
> Reconciliation HARD GATE: **PASSED — 0 contradictions** (no DISCUSS/DEVOPS for this feature;
> requirements INHERITED per the parent provisioning feature; only DESIGN wave-decisions exist and
> D1–D6 are RATIFIED 2026-06-13, internally consistent).

## Scenario catalogue (11 scenarios; 1 walking skeleton + 10 @pending)

| # | Scenario | Tags | Category | Driving port |
|---|----------|------|----------|--------------|
| 1 | A super-admin provisions a new isolated workspace from the browser | `@walking_skeleton @wiring_e2e @us-mwt07` | happy (WS) | `POST /admin/instance/workspaces` |
| 2 | The instance dashboard shows the workspace list and the provision and grant forms | `@pending @us-mwt07` | happy | `GET /admin/instance/workspaces` |
| 3 | A super-admin grants super-admin to another operator from the browser | `@pending @us-mwt07` | happy | `POST /admin/instance/super-admins` |
| 4 | Granting super-admin twice from the browser is idempotent | `@pending @us-mwt07` | edge (idempotence) | `POST /admin/instance/super-admins` |
| 5 | Granting an unknown email does not reveal whether the user exists | `@pending @us-mwt07 @error` | error (non-enum) | `POST /admin/instance/super-admins` |
| 6 | A signed-out request to the admin surface is refused like a path that never existed | `@pending @us-mwt08 @error` | error (non-enum) | all 3 routes (GET + 2 POST) |
| 7 | A signed-in non-super-admin request to the admin surface is refused non-enumerably | `@pending @us-mwt08 @error` | error (non-enum) | all 3 routes (GET + 2 POST) |
| 8 | A provision request without a valid security token is refused | `@pending @us-mwt07 @error` | error (CSRF) | `POST /admin/instance/workspaces` |
| 9 | The legacy create-workspace route no longer exists | `@pending @us-mwt07 @error @verify-path-unchanged` | error (D3 retire) | legacy `POST /workspaces` |
| 10 | Provisioning from the browser leaves existing workspaces untouched | `@pending @us-mwt07 @us-mwt08` | happy (isolation) | `POST /admin/instance/workspaces` |
| 11 | The browser-provisioned workspace is a real isolated tenant | `@pending @us-mwt08` | happy (isolation) | `POST` then `resolve_active_workspace` seam |

**Error/edge ratio**: 6 of 11 scenarios carry `@error` or test an edge/idempotence boundary
(#4, #5, #6, #7, #8, #9) = **55%** — well above the 40% mandate. Appropriate for a
security-defining surface whose entire reason for the design's rigor is the non-enumerable gate.

## Driving adapter coverage (RCA-fix P1 — every route exercised via its protocol)

| Route (DESIGN D1) | Method | Scenario(s) exercising it via real HTTP |
|---|---|---|
| `/admin/instance/workspaces` | GET | #2 (renders), #6/#7 (refused non-enumerably) |
| `/admin/instance/workspaces` | POST | #1 (WS provision), #8 (CSRF-less refused), #10 (untouched), #6/#7 (refused), #11 (provision) |
| `/admin/instance/super-admins` | POST | #3 (grant), #4 (idempotent), #5 (non-enum unknown email), #6/#7 (refused) |
| legacy `/workspaces` | POST | #9 (retired — refused as never-existed, no 409) |

Zero uncovered entry points. The GET page, both POSTs, and the retired legacy POST each have at
least one scenario reaching them over the real in-process HTTP port (not a direct service call).

## Adapter coverage table (Mandate 6 — every driven adapter has a @real-io scenario)

| Driven adapter | @real-io scenario | Covered by | Status |
|---|---|---|---|
| `Services::provision_workspace` (+ atomic create+seed tx) | YES | #1, #10, #11 | REUSE (shipped) |
| `is_instance_admin` authz (the gate result) | YES | #1 (pass), #6/#7 (refuse) | REUSE (shipped) |
| `grant_instance_admin` (idempotent) | YES | #3, #4 | REUSE (shipped) |
| `user_id_by_email` (grant resolve) | YES | #3, #5 | REUSE (shipped) |
| `list_workspaces` (NEW thin non-tenant-scoped read, D4) | YES | #2 (dashboard list) | CREATE NEW (thin) |
| real `foundry_session` (tower-sessions PG store) | YES | every signed-in/out scenario | REUSE (shipped) |
| `csrf_middleware` (double-submit) | YES | #1/#3 (valid token), #8 (missing token refused) | REUSE (shipped) |
| `resource_not_found_page()` (uniform 404) | YES | #6, #7, #9 | REUSE (shipped) |
| `InviteToken` + invite-url builder | YES | #1 (link rendered, D5 — informational) | REUSE (shipped) |

Zero "NO — MISSING" rows. The single genuinely-new artifact under test is the thin `list_workspaces`
read (#2) plus the `instance_admin.rs` adapter itself (every scenario). Everything else is the
SHIPPED, mutation-hardened backend reused verbatim.

## RED-state contract per scenario (Mandate 7 — crate COMPILES → not BROKEN; RED = MISSING_FUNCTIONALITY)

The crate COMPILES (Gherkin is text; `acceptance.rs` is NOT edited; no undefined symbol is added).
Genuine RED at runtime against real testcontainers PG16:

| # | RED cause (what is missing) | Greens when DELIVER… |
|---|---|---|
| 1 | `instance_admin.rs` adapter + `POST …/workspaces` route + success template absent | ships the adapter + route + template; the provision form drives the shipped use-case |
| 2 | `GET …/workspaces` page + `list_workspaces` read + dashboard template absent | ships the GET handler + thin read + page template |
| 3 | `POST …/super-admins` route + grant handler absent | ships the grant handler over the shipped `grant_instance_admin` + `user_id_by_email` |
| 4 | grant handler absent (idempotence inherited from shipped `ON CONFLICT DO NOTHING`) | ships the grant handler; idempotence is green-by-inheritance |
| 5 | grant handler + non-committal unknown-email mapping (D2 (g)) absent | ships the handler returning the SAME confirmation shape for known/unknown emails |
| 6 | `require_instance_admin` gate absent — no uniform 404 for signed-out | ships the gate returning `resource_not_found_page()` for the no-SessionUser arm |
| 7 | gate absent — no uniform 404 for non-super-admin | ships the gate; the non-admin arm returns the BYTE-IDENTICAL 404 |
| 8 | route absent (so CSRF layer never reached) | ships the route UNDER `csrf_middleware`; CSRF-less POST refused, no workspace created |
| 9 | legacy `POST /workspaces` STILL 409s | DELETES the route + handler (D3); the path returns the never-existed refusal |
| 10 | provision route absent | ships the route; "untouched" is green-by-inheritance off the shipped tx |
| 11 | provision route absent | ships the route; isolation is green-by-inheritance off slices 1-6 |

Per Mandates 9 + 11: LAYER-3 real-adapter scenarios are **example-based** (NOT property-based);
every sad / evil-user / unauthorised path is enumerated explicitly (#5, #6, #7, #8, #9); no PBT
machinery at this layer. Mandate 8 state-delta is layers 1-3 with a Python pilot port; no
`state_delta.rs` Rust port exists (matching slices 1-6), so LAYER-3 assertions are traditional over
port-exposed web observables. **No Tier B** state-machine PBT (the journey is example-coverable; the
input space is not domain-rich enough to warrant generative exploration — the security property is a
finite 3-route × 2-refusal-cause matrix, fully enumerated in #6/#7).

## Decision → scenario traceability (D1–D6)

| Decision (DESIGN) | Exercised by | Note |
|---|---|---|
| **D1** routes/screens (GET page + 2 POSTs, htmx) | #1 (POST provision), #2 (GET page + forms), #3 (POST grant) | full surface covered |
| **D2** inline `require_instance_admin` gate + uniform 404 + non-committal grant | #5 (grant non-enum), #6 (signed-out 404), #7 (non-admin 404, byte-identical) | the security core; ADR-002 response-mapping table asserted byte-identically |
| **D3** RETIRE the legacy `POST /workspaces` 409 | #9 | asserts the route is GONE (refused as never-existed, no 409) — RATIFIED RETIRE/DELETE |
| **D4** thin adapter over the SHIPPED use-case + thin `list_workspaces` | #1/#10/#11 (use-case), #2 (`list_workspaces`) | no new domain/store logic beyond the thin read |
| **D5** invite-accept OUT of v1 (link informational) | #1/#11 | #1 asserts the link is RENDERED (not followed); #11's "first admin acts" rides `resolve_active_workspace`, NOT a live accept flow — **honoured: no sign-in-via-link scenario authored** |
| **D6** LAYER-1e allow-list line (`instance_admin`) | (build-time, non-testable-at-this-layer) | a `cargo xtask check-arch` build-time guard, NOT an acceptance behaviour. Noted for DELIVER; not an AT. |

**D6 is explicitly noted non-testable-at-this-acceptance-layer**: it is a build-time architecture
guard (the new file stem must be allow-listed so the LAYER-1e detector stays precise), enforced by
`cargo xtask check-arch`, not by an HTTP-port acceptance scenario. DELIVER adds the one line; the
xtask gold test (if any) is the verification surface, consistent with how slices 1-4 treated the
LAYER-1e rule.

## Story → scenario traceability (US-MWT07 / US-MWT08 web legs)

| Story | Web leg covered by | 
|---|---|
| **US-MWT07** (super-admin provisions a new workspace; grant super-admin) | #1 (provision WS), #2 (dashboard), #3 (grant), #4 (grant idempotent), #5 (grant non-enum), #8 (CSRF), #9 (sole path), #10 (untouched) |
| **US-MWT08** (the provisioned tenant honours the isolation boundary; non-super-admin refused) | #6 (signed-out refused), #7 (non-admin refused), #10 (untouched/isolated), #11 (real isolated tenant) |

Both inherited stories have web-leg coverage. No story ID is uncovered.

## Earned-Trust (probe-don't-assume) commitments → scenarios (architecture.md §7)

| DESIGN commitment | Scenario | Litmus |
|---|---|---|
| Non-enumerability PROBED (signed-out AND non-admin get byte-identical 404s on every route) | #6 + #7 | revert-reds-it: collapsing the two refusal arms into distinct responses (a 401/403) MUST re-RED the byte-identity assertion (DELIVER) |
| CSRF PROBED (missing/mismatched `_csrf` refused) | #8 | the shipped double-submit middleware refuses; no workspace created |
| No new domain regression (existing suite stays green) | green-before/green-after invariant | the feature adds an adapter, not domain logic; slices 1-6 stay green |

The "defence-in-depth: use-case re-checks the gate even with the adapter gate bypassed" commitment is
covered by the SHIPPED gate-inversion mutant on `Services::provision_workspace` (slice-06 scope) —
NOT re-authored here (it is not a web-adapter behaviour; bypassing the adapter gate to probe the
use-case is a unit/service-level test, not an HTTP-port acceptance scenario for THIS feature).
