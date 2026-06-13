# ADR-002 — Web authz gate + non-enumerability (+ CSRF/session)

## Status
**PROPOSED** (2026-06-12). DESIGN wave, Propose mode. Awaiting ratification (D2 is a grounded
default dictated by the shipped seams; low contention).

## Context
The web provisioning surface (ADR-001) is the highest-privilege browser surface in the product
(it mints workspaces and grants instance authority). It must be reachable ONLY by an instance
super-admin, and an unauthorized user must not even learn the surface EXISTS — consistent with the
shipped tenancy boundary, which refuses cross-tenant reaches with a UNIFORM 404 (no 403-vs-404
oracle). The original parent ADR-004 said "session + CSRF, gated by `is_instance_admin`"; this ADR
firms HOW the gate is enforced and HOW non-enumerability is achieved, grounded in shipped code.

Grounding (read the code):
- The web auth idiom is **inline session-extract, NOT middleware** (`bootstrap.rs`, `signin.rs`,
  `admin_tokens.rs`): each handler reads `session.get::<SessionUser>(SESSION_KEY_USER_ID)` and acts.
  There is no auth-required layer to extend.
- `/workspace/switch` (`session.rs:138-191`) is the shipped non-enumerable, fail-closed precedent:
  `set_active_workspace` returns `Ok(false)` for a non-member ⇒ the handler returns
  `resource_not_found_page()` (`bootstrap.rs:382-388`) — the SAME uniform 404 a foreign-resource
  reach returns. No status/body oracle distinguishes "not a member" from "doesn't exist."
- `csrf_middleware` (`csrf.rs:96-173`) is a router layer that double-submit-checks every non-safe
  method (cookie vs `_csrf` form field / `hx-csrf` header), 403 on mismatch.
- `is_instance_admin(user_id)` is the shipped, mutation-hardened authz predicate in services/store.

## Options considered
### Gate enforcement
- **(a) Inline `require_instance_admin` gate in a shared helper/extractor (RECOMMENDED).** Each
  `/admin/instance/…` handler calls a small helper that reads `SessionUser`, calls
  `store.is_instance_admin`, and returns the resolved user or the uniform 404. Matches the codebase's
  inline idiom; one choke point; trivially testable.
- **(b) A new tower middleware layer gating the `/admin/instance` prefix.** Cleaner in theory, but
  the codebase gates inline everywhere; introducing a one-off middleware tier for three routes adds
  a new pattern to maintain and a path-prefix matching surface, for no benefit over (a). Rejected.
- **(c) Reuse the use-case gate only (no adapter gate).** Let `Services::provision_workspace`'s own
  `Forbidden` be the sole gate. Rejected: GET (the list page) has no use-case to gate it, and a
  Forbidden mapped to an HTTP status would leak a 403 oracle unless re-mapped — (a) is simpler and
  uniformly non-enumerable. (The use-case gate REMAINS as defence-in-depth.)

### Non-enumerability (the response on refusal)
- **(d) Uniform 404 via `resource_not_found_page()` for BOTH signed-out and non-super-admin
  (RECOMMENDED).** Identical status + body — no oracle. Mirrors `/workspace/switch` and the tenancy
  boundary exactly.
- **(e) 401 for signed-out, 404 for non-admin.** Rejected: a 401-vs-404 split tells an attacker that
  the surface exists for *someone*, which is exactly the enumeration the boundary forbids.
- **(f) 403 for non-admin.** Rejected outright: a 403 confirms the surface exists — the classic
  enumeration oracle the shipped boundary was designed to eliminate.

### Grant action + user enumeration
- **(g) Non-committal grant result for unknown emails (RECOMMENDED).** A grant POST for an email
  that resolves to no user returns the SAME confirmation shape as a successful grant (or a generic
  "if that user exists, they are now a super-admin"), so the grant form is not a user-existence
  oracle. (The grant itself is idempotent.)
- **(h) Explicit "no such user" error.** Rejected: turns the grant form into a user-enumeration
  oracle on the highest-privilege surface.

## Decision
**Gate: (a) inline `require_instance_admin` helper. Non-enumerability: (d) uniform 404 via
`resource_not_found_page()` for signed-out AND non-super-admin alike. Grant: (g) non-committal
result for unknown emails.** CSRF + session: the new routes mount UNDER the SHIPPED `csrf_middleware`
+ `session_layer` (no change to either); each form view carries a `csrf: String` rendered into
`<input type="hidden" name="_csrf">`. The use-case's own `is_instance_admin` re-check stays as
defence-in-depth (it already exists and is mutation-hardened).

## Response mapping (the non-enumerability contract — explicit for DISTILL/DELIVER)
The `require_instance_admin` helper resolves to EXACTLY ONE of two outcomes, and BOTH refusal cases
return the SAME `resource_not_found_page()` (identical status 404 + identical body):

| Caller state | Helper outcome | HTTP response |
|---|---|---|
| No `SessionUser` in session (signed-out) | refusal | `resource_not_found_page()` — uniform 404 |
| `SessionUser` present, `is_instance_admin(user_id) == false` | refusal | `resource_not_found_page()` — uniform 404 (BYTE-IDENTICAL to the signed-out case) |
| `SessionUser` present, `is_instance_admin(user_id) == true` | pass | the resolved `SessionUser` returned to the handler |

There is no third response shape and no branch that varies status/body by *which* refusal occurred —
that is the non-enumerability property. DELIVER MUST assert it via a **revert-reds-it litmus**:
collapsing the two refusal arms into distinct responses (e.g. a 401 or 403 for one) must re-RED the
acceptance assertion that a signed-out user and a signed-in non-super-admin receive byte-identical
404s on every `/admin/instance/…` route.

## Consequences
- **Positive**: the surface is non-enumerable by construction (one uniform 404, no oracle); the gate
  reuses the shipped fail-closed idiom and the shipped authz predicate; CSRF/session are inherited
  byte-for-byte; the use-case re-check means even a future adapter bug cannot provision for a
  non-super-admin.
- **Negative**: a legitimate super-admin who is signed OUT gets a 404 rather than a sign-in redirect
  (acceptable — uniformity is the security property; they sign in via `/sign-in` then retry, and the
  surface is documented to the operator out-of-band).
- **Security**: defining property. No 403-vs-404 oracle; no user-enumeration oracle on grant; CSRF on
  every state-changing POST; defence-in-depth gate at the use-case.

## Relationship
Firms `multi-workspace-tenancy` ADR-004's "session + CSRF, gated by `is_instance_admin`" into a
concrete, grounded mechanism, and extends the shipped tenancy non-enumerability boundary
(NFR-MWT-SEC-02) to the instance-admin surface. Inherits D6 (the LAYER-1e allow-list line, ADR-004).
</content>
