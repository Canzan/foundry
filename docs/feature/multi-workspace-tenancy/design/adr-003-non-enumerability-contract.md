# ADR-003 — Non-enumerability contract (uniform foreign-id ≡ missing-id)

## Status
Proposed.

## Context
A cross-tenant request must reveal NOTHING about the other tenant — not even that a resource exists
(NFR-MWT-SEC-02, DM2). A single surface where "exists but forbidden" (403) differs from "does not
exist" (404), or where body shape / timing differs, is an enumeration oracle. The shipped
`attachments.rs find_attachment_in_workspace(id, requester_workspace_id)` already does this for
attachments: `WHERE id=$1 AND workspace_id=$2` returns `None` whether the row is missing OR belongs
to another workspace — "missing" and "not yours" collapse. This is the canonical idiom to
generalize uniformly.

## Options considered
- **(a) Per-resource ad-hoc choice of 403/404.** Rejected: guarantees an oracle somewhere; the very
  failure mode DM2 forbids.
- **(b) Generalize the shipped `find_*_in_workspace` → `None` idiom as the SINGLE refusal pattern**
  for every tenant-scoped resource, and pin the observable response per surface to ONE shape.
- **(c) A dedicated `WorkspaceScoped<T>` error type carrying a forced-uniform status.** More
  machinery than needed; (b) achieves the same with the shipped idiom.

## Decision
**(b).** The single refusal idiom every surface reuses:
- **Store layer**: every tenant-scoped single-resource lookup is `find_*_in_workspace(id,
  acting_workspace_id)` returning `Option`, where `None` is returned identically for a missing id
  and a foreign id (generalizing `attachments.rs`). No store method returns a "forbidden because it's
  another workspace's" signal.
- **Web htmx tier**: `None` → the SAME not-found response (status + page body) as a never-existed
  id. No 403 for cross-tenant resource access (admin-action authz failures — ADR-004/the shipped
  `is_workspace_admin` gate — are a separate, intra-workspace concern and keep their shipped shape).
- **JSON `/api/v1`**: `None` → the SHIPPED `status_for` 404 JSON envelope, byte-identical to a
  never-existed id; token revoke of a foreign jti is already a non-enumerable `NotFound`
  (`tokens.rs`, reused as-is).
- **Timing**: the foreign-id and missing-id paths execute the same query (`WHERE id AND
  workspace_id`), so they share a timing profile by construction — no extra branch that could leak.

Slice 4 hardens this into an explicit adversarial matrix (web reads/writes, admin actions, API
reads, token revoke) asserting foreign-id ≡ missing-id with no status/body/timing/shape oracle.

## Consequences
- **Positive**: one idiom, already shipped + tested, reused everywhere; no per-resource decision to
  get wrong; the timing-equivalence is structural (same query), not a fragile constant-time hack.
- **Negative**: collapsing 403→404 for cross-tenant access can be marginally confusing to a *legit*
  user who genuinely lost access — accepted, because distinguishing the two IS the oracle. Intra-
  workspace authz failures retain their normal 403, so the legit-user experience inside their own
  workspace is unchanged.
- **Boundary**: this ADR governs *cross-tenant resource* refusals. It does NOT change the shipped
  non-enumerable sign-in error or the bearer 401 — those are unchanged invariants.

## Slice alignment
Generalized incrementally with each surface (Slice 2 web, Slice 3 API), then proven uniform across
all surfaces in Slice 4 (the explicit non-enumerability hardening slice).
