# Story Map — Token-Management API

## User: Sven Aarø (integrator) / the Acme rotation job (automation) / Dana's audit pipeline (security automation)
## Goal: inventory and lifecycle-manage machine-token credentials PROGRAMMATICALLY over /api/v1, with the escalation surface bounded

> Sequenced **safest-authz-first**: prove the ratified authz model (Q-AUTHZ → option c) on the
> SAFEST real op (read-only LIST) end-to-end before any mutation, then revoke (incl. self), then
> harden the refusal boundary. Programmatic MINT (the escalation-sensitive op) is DEFERRED out of
> v1 entirely — see `wave-decisions.md` and `out-of-scope.md`. Reflects the recommended model; if
> the user ratifies (a) or (c)+(b), a MINT slice is appended with the corresponding capability gate
> + guardrails.

## Backbone

| Authenticate (SHIPPED) | Inventory | Revoke / Rotate | Trust the contract |
|------------------------|-----------|-----------------|--------------------|
| bearer → Principal::Machine | GET list as JSON | revoke a jti | stable error codes |
| (MachinePrincipal extractor) | authz: mgmt vs non-mgmt | revoke-SELF (rotation) | non-enumerable refusals |
| fail-closed identical 401 | value-free TokenView | effective next request | cross-workspace 404 |
|  |  | idempotent re-revoke | rate guardrail (SEC-07) |

---

### Walking Skeleton (thinnest end-to-end slice across all activities)

One task per backbone activity, the minimum that proves the WHOLE thing works and the authz model
is real:

- **Authenticate**: reuse the SHIPPED `MachinePrincipal` bearer extractor (no new work).
- **Inventory**: `GET /api/v1/.../tokens` returns the workspace's tokens as a value-free JSON array
  **for a management-capable (admin-bound) bearer**, and **refuses a non-management bearer with a
  non-enumerable 403** — this single slice PROVES the ratified authz model on the safest op.
- **Revoke / Rotate**: not yet (Slice 2).
- **Trust the contract**: the LIST already rides the SHIPPED `status_for` envelope; the 200 shape +
  the 403 refusal shape are asserted here.

The skeleton is US-TMA00 (route-group scaffold + authz-gate seam, `@infrastructure`, folded) +
US-TMA01 (GET list, authorized vs refused). It is the smallest thing that answers the riskiest
question — "is a management-capable token authorized and a non-management token refused?" — without
shipping any mutation.

### Release 1 (Walking Skeleton): "I can see what exists, and the authz model is proven"
- US-TMA00 — `/api/v1/.../tokens` route-group scaffold + the authz-gate seam (`@infrastructure`,
  folded into the skeleton, never standalone).
- US-TMA01 — `GET .../tokens` lists the workspace's tokens as value-free JSON for a management
  bearer; refuses a non-management bearer (403, non-enumerable).
- **Target outcome**: mt-api-job-1 (audit/inventory as JSON). KPI: a security-automation caller
  pulls the registry as JSON with zero browser/DB access; authorized-vs-refused proven.
- **Rationale**: read-only, no escalation, no mutation — the safest place to ratify the authz model
  end-to-end. Riskiest-assumption-first: the authz model is the riskiest assumption, and LIST
  exercises it without any blast radius.

### Release 2: "I can kill and rotate credentials hands-free"
- US-TMA02 — revoke a token via the API; it is refused on its next `/api/v1` call (idempotent;
  cross-workspace/unknown → non-enumerable 404).
- US-TMA03 — revoke-SELF: a token disables its own future use (the rotation flow).
- **Target outcome**: mt-api-job-2 (rotate/revoke programmatically). KPI: a rotation job revokes
  (incl. self) hands-free and proves the credential is dead on the next call.
- **Rationale**: revoke is a mutation but NOT self-amplifying (worst case is a loud, reversible,
  workspace-confined DoS — never a credential leak). Revoke-self is the safest mutation (a token
  retiring itself), so US-TMA03 is the gentlest entry into mutation. Sequenced after LIST so the
  authz gate is already proven.

### Release 3: "I can build an SDK against a predictable, hostile-input-safe contract"
- US-TMA04 — stable error contract across the token routes (codes/statuses from `status_for`;
  read-after-write equality on the LIST shape).
- US-TMA05 — the refusal boundary: non-management caller (403), cross-workspace jti (non-enumerable
  404), unknown jti (identical 404), revoked/expired/forged bearer (identical 401) — the evil-caller
  scenarios; plus the SEC-07 rate guardrail on a revoke storm.
- **Target outcome**: mt-api-job-3 (predictable machine-readable answers) + mt-api-job-1's
  non-enumerability guarantee. KPI: 100% of adversarial calls refuse non-enumerably; every refusal is
  a stable `error.code` an integrator can branch on.
- **Rationale**: hardens what Releases 1-2 established. Pulls the confirmation-bias defense
  (evil-caller, hostile input) into explicit, testable scenarios so the contract is provably safe to
  publish.

### DEFERRED (NOT in v1): "provision a new credential programmatically (mint via API)"
- mt-api-job-4 / a future US-TMA06. **Out of v1** under the ratified authz model: a bearer that can
  MINT self-replicates (the mint loop). If the user ratifies programmatic mint, it becomes its own
  slice WITH option (b)'s explicit `tokens:manage` capability + the SEC-07 mint-rate guardrail +
  "management tokens cannot mint management tokens". Documented in `out-of-scope.md`.

## Priority Rationale

Priority = **Value** (outcome impact) × **Urgency** (de-risks the riskiest assumption) / **Effort**.

| Priority | Slice | Target outcome | KPI link | Rationale |
|----------|-------|---------------|----------|-----------|
| 1 | Walking Skeleton (US-TMA00, US-TMA01) | End-to-end LIST works; authz model proven | mt-api-job-1 | **Riskiest assumption first.** The authz/escalation model (Q-AUTHZ) is THE risk; LIST exercises it with zero blast radius (read-only). Lowest effort (use-cases + envelope + extractor all SHIPPED), highest urgency (de-risks the crux). |
| 2 | Release 2 (US-TMA02, US-TMA03) | Hands-free revoke + rotation | mt-api-job-2 | Highest-value behaviour change after inventory (rotation + incident response go hands-free). Revoke is non-self-amplifying, so it ships safely once the authz gate is proven. Revoke-self (US-TMA03) is the gentlest mutation and the core of rotation. |
| 3 | Release 3 (US-TMA04, US-TMA05) | Predictable + hostile-input-safe contract | mt-api-job-3 | Hardens the surface for publication. Lower marginal value (the envelope is SHIPPED) but essential for an integrator to trust + script against; the evil-caller scenarios close the non-enumerability + escalation-boundary requirements. |
| — | DEFERRED (mint, mt-api-job-4) | Programmatic provisioning | mt-api-job-4 | **Held out of v1 by the escalation risk**, not by effort. Requires its own ratification (option c+b) + capability claim + mint-rate guardrail before it can ship safely. Highest risk / not the largest current bottleneck (audit + rotation are). |

> Each slice touches MULTIPLE backbone activities (slices are outcome-based, not feature-grouped):
> the skeleton touches Authenticate + Inventory + Trust-the-contract; Release 2 adds Revoke/Rotate +
> re-exercises Authenticate; Release 3 hardens Trust-the-contract across all of them.
