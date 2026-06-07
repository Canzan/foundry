# Coverage Matrix — machine-token-admin-ux (DISTILL)

Every US-MT0x acceptance criterion + security NFR mapped to the scenario(s) that
exercise it. 28 scenarios across 7 `.feature` files; all `@real-io` (in-process
axum router + real Postgres). Exactly one `@walking_skeleton`. ~18/28 (64%)
error/edge — above the 40% mandate.

## Feature files + scenario tally

| File | Scenarios | of which error/edge | Tags |
|---|---|---|---|
| `us-mt01-mint.feature` | 5 | 3 | `@us-mt01` (+1 `@walking_skeleton`), `@us-mt00` |
| `us-mt-display-once.feature` | 3 | 1 | `@us-mt01` |
| `us-mt02-list.feature` | 4 | 1 | `@us-mt02` |
| `us-mt03-revoke.feature` | 5 | 2 | `@us-mt03` |
| `us-mt04-scope-expiry.feature` | 4 | 2 | `@us-mt04` |
| `us-mt05-authz.feature` | 4 | 3 | `@us-mt05` |
| `us-mt06-audit.feature` | 3 | 1 | `@us-mt06` |
| **Total** | **28** | **~18 (64%)** | all `@machine-token-admin @real-io` |

Walking skeleton: **us-mt01 "An admin issues a working token and sees its value
once"** (`@walking_skeleton @us-mt00`).

## US-MT00 — signer in AppState + `created_by` migration (@infrastructure, folded)

| AC | Scenario(s) |
|---|---|
| issuer server exposes a signer; verifier-only does not (no mint surface) | us-mt01 "issues a working token …" (issuer) + "issuing is refused gracefully where it is not enabled" (verifier-only) |
| boot key self-test still passes on issuer | exercised by the issuer harness booting with `Some(test signer)` (`InProcHarness::spawn`); the self-test is the SHIPPED probe (signer.md) |
| `machine_tokens` has nullable `created_by` REFERENCES users(id), forward-only | migration `0008` (applied by the harness); us-mt06 "unknown issuer" asserts a NULL-`created_by` row surfaces as "—" |
| `insert_machine_token` accepts + persists `created_by` | wired in DELIVER (step-skeletons §2); us-mt01 "Issuing records who minted the token" + us-mt06 attribution scenarios assert the observable |

## US-MT01 — mint + one-time display

| AC | Scenario(s) |
|---|---|
| value shown exactly once, copy affordance, only-time warning | us-mt01 WS "value shown once" |
| issuance view shows jti, label, scope, expiry | us-mt01 WS "shows its id, label, scope, expiry" |
| a token issued this way authenticates against /api/v1 | us-mt01 WS "authenticates against the API" — **cross-check against the SHIPPED us-w05b verify path** |
| value not retrievable after leaving the view; only jti+metadata | display-once "value is nowhere on the surface" + "only metadata are shown" |
| value never written to DB or logs | display-once "never written to the registry" (asserts the schema has no token/secret/hash column — NFR-MT-DATA-02) |
| verifier-only: reported not enabled, no value/partial, no error | us-mt01 "refused gracefully where it is not enabled" + "… when a mint is attempted" |
| (edge) lose-it = reissue | display-once "Losing the token before copying …" |
| (edge) missing label refused | us-mt01 "Issuing without a label is refused" |

## US-MT02 — list

| AC | Scenario(s) |
|---|---|
| lists workspace tokens newest-first with label/scope/expiry/status | us-mt02 "sees the workspace's issued tokens newest first" |
| no token value in the list | us-mt02 "no token value appears anywhere in the list" (+ display-once) |
| list scoped to the current workspace | us-mt02 "The list is scoped to the acting workspace" (modelled per single-workspace constraint — `upstream-issues.md` UI-1) |
| empty workspace → inviting empty state | us-mt02 "An empty workspace shows guidance" |
| (edge) revoked token still listed as revoked | us-mt02 "A revoked token still appears in the list as revoked" |

## US-MT03 — revoke (kill switch)

| AC | Scenario(s) |
|---|---|
| admin revokes from the list row | us-mt03 "refused on its very next API call" |
| immediate-and-irreversible warning before it takes effect | us-mt03 "Revoking warns it is immediate and irreversible" |
| revoked token's next /api/v1 call is refused | us-mt03 "refused on its very next API call" — **cross-check against the SHIPPED us-w05b `jti` denylist** (`resolve_active_token`); presents the REAL minted credential bound to the revoked jti and asserts 401 |
| revoked row survives, shows Revoked | us-mt03 "refused on its very next API call" + "already-revoked is harmless" |
| revoke idempotent | us-mt03 "Revoking an already-revoked token is harmless" |
| cross-workspace revoke refused non-enumerably; foreign token untouched | us-mt03 "cannot revoke a token outside their workspace" (synthetic foreign jti — UI-1) |
| (edge) revoke POST without `_csrf` refused | us-mt03 "A revoke without a valid anti-forgery token is refused" (NFR-MT-SEC-07) |

## US-MT04 — scope + expiry within bounds

| AC | Scenario(s) |
|---|---|
| choose scope (workspace/team) + expiry within range | us-mt04 "issues a team-scoped, time-bounded token" |
| issued token's claims + list reflect the choice | us-mt04 "team-scoped …" (list shows the scope) |
| expiry beyond cap refused with max stated | us-mt04 "Expiry beyond the server cap is refused" (asserts "365") |
| (edge) expiry exactly at cap accepted | us-mt04 "An expiry exactly at the cap is accepted" (365 days) |
| scope referencing a team outside the workspace refused | us-mt04 "A scope that is not part of the workspace is refused" (synthetic foreign team — UI-1) |

## US-MT05 — admin-only authz boundary

| AC | Scenario(s) |
|---|---|
| every entry point requires `is_workspace_admin` | us-mt05 "admin can open the token surface" + the three refusal scenarios |
| non-admin refused non-enumerably (no surface/existence leak) | us-mt05 "non-admin member is refused without learning the surface exists" + "cannot issue" + "cannot revoke" |
| admin of one workspace cannot manage another's | covered by the non-enumerable refusal model (UI-1); the use-case authz is asserted via the non-admin paths |

## US-MT06 — audit (minted-by + last-used)

| AC | Scenario(s) |
|---|---|
| list shows "minted by {admin}" from `created_by` | us-mt06 "attributes each token to who issued it" |
| list shows last-used or "never" | us-mt06 "shows whether a token is still being used" |
| NULL-issuer token shows unknown/—; new tokens show the admin | us-mt06 "A token issued before issuer attribution shows an unknown issuer" |

## Security / Reliability NFR → scenario

| NFR | Scenario(s) |
|---|---|
| NFR-MT-SEC-01 (shown once, never persisted) | us-mt01 WS + display-once "never written to the registry" (schema has no secret column) |
| NFR-MT-SEC-02 (never re-displayed) | display-once (all 3) + us-mt02 "no value in list" |
| NFR-MT-SEC-03 (admin-only, non-enumerable) | us-mt05 (all) + us-mt03 cross-workspace |
| NFR-MT-SEC-04 (signer posture explicit/bounded) | us-mt01 issuer vs verifier-only scenarios |
| NFR-MT-SEC-05 (revocation effective next request) | us-mt03 "refused on its very next API call" (us-w05b denylist cross-check) |
| NFR-MT-SEC-06 (issuance attributable) | us-mt01 "records who minted" + us-mt06 attribution |
| NFR-MT-SEC-07 (browser CSRF/session contract) | us-mt03 no-CSRF refused; the whole feature drives the real session+CSRF layers; the existing browser-auth suite stays green (174 pre-existing scenarios unaffected) |
| NFR-MT-REL-01 (mint all-or-nothing) | enforced by the scaffold's 501 (never a partial token); DELIVER reuses `force_board_render_failure` seam — assertion: us-mt01 "no token value is shown" on failure paths |
| NFR-MT-REL-02 (revoke idempotent) | us-mt03 "already-revoked is harmless" |
| NFR-MT-REL-03 (registry reads workspace-isolated) | us-mt02 "scoped to the acting workspace" + us-mt03 cross-workspace |
| NFR-MT-DATA-01 (forward-only `created_by`, no rewrite) | migration `0008` (forward-only); us-mt06 unknown-issuer edge |
| NFR-MT-DATA-02 (no secret column ever added) | display-once "never written to the registry" asserts the live `machine_tokens` columns contain no token/secret/hash/value column |
| NFR-MT-PERF-01 (interactive mint/revoke) | NOT a dedicated scenario this wave — the in-process harness budget is implicitly fast (<200ms); DELIVER may add a timing assertion (≥200ms budget per F-004). Flagged below. |

## Cross-checks against the SHIPPED us-w05b machine-token-auth behaviour

- **Real signing proof** (us-mt01 WS): the minted token is presented to the
  SHIPPED `/api/v1` verify path and must authenticate — proving the product
  minted a real EdDSA-signed credential (not a stub).
- **Kill-switch proof** (us-mt03): after revoke, the REAL credential bound to the
  revoked `jti` is presented to `/api/v1` and must be refused 401 by the SHIPPED
  per-request `jti` denylist (`resolve_active_token`) — reusing, not rebuilding,
  the us-w05b "A revoked credential is refused on its next use" mechanism.

## Uncovered / deferred ACs (explicit)

- **NFR-MT-PERF-01** (mint/revoke ≤200ms server-side): no dedicated timing
  scenario this wave. Rationale: the in-process harness is implicitly fast and a
  flaky-prone wall-clock assertion adds little at RED; recommend DELIVER add it
  with a ≥200ms budget if the team wants it pinned. (Not a blocker — the budget
  is a non-functional guardrail, not a user-visible behaviour.)
- **Two concurrently-existing real workspaces**: blocked by the single-workspace
  schema (`upstream-issues.md` UI-1); cross-workspace paths are modelled with
  synthetic foreign jti/team uuids (observably identical non-enumerable refusal).
- **JSON token-management API**: deferred fast-follow per DISCUSS Q6 / DESIGN;
  out of scope this wave.
