# ADR-009: CSRF Coverage Extends to PATCH and DELETE

## Status
Accepted — 2026-05-25

## Context

Slice 5 introduces two new state-mutating HTTP verbs on the comments
resource: `PATCH /…/comments/{id}` (edit) and
`DELETE /…/comments/{id}` (delete). The slice-1 CSRF middleware
(`csrf::csrf_middleware`) implements a double-submit pattern: a
`_csrf` token on the session is matched against either a `_csrf` form
field (for `application/x-www-form-urlencoded` bodies) or an `HX-CSRF`
header (for htmx requests). The middleware is registered layer-wide in
`build_router`, so it automatically applies to any new state-mutating
route.

The question is whether PATCH and DELETE need any special treatment, or
whether they inherit the existing middleware behaviour cleanly. One
detail is worth recording: htmx's `hx-delete` attribute fires a DELETE
request with an **empty body** by default — there is no form field to
carry `_csrf` in. The token MUST ride in the `HX-CSRF` header for
DELETE.

## Decision

PATCH and DELETE **inherit the existing CSRF middleware unchanged**. No
new middleware. No per-route opt-in. No exemption.

The one-line clarification:

- **PATCH** with `application/x-www-form-urlencoded` body: token rides
  in the `_csrf` form field (same as POST).
- **DELETE** with `hx-delete` (empty body): token rides in the
  `HX-CSRF` header. The htmx integration hook in slice 2 already sets
  this header on all htmx requests; no client-side change required.

The middleware order is preserved: session middleware extracts the
session token first; CSRF middleware compares the session token to the
request-supplied token; handler runs only on match. Non-matching
requests get a 403 with no handler dispatch.

## Alternatives Considered

### A: Inherit existing middleware (chosen)
See Decision.

### B: Bypass CSRF for DELETE (or PATCH)
Some frameworks exempt "safe" verbs from CSRF; PATCH and DELETE are not
safe under RFC 7231 (both have side effects) but a misreading of
"idempotent" sometimes leads to exemption.

- **Pros**: One less header to wire.
- **Cons**: PATCH and DELETE are explicit attack targets for
  cross-origin form abuse (`<form method="DELETE">` is not a real
  browser feature, but an `<img src="..." onerror>` can fire arbitrary
  fetch via JS in an XSS context; the CSRF token blocks this when the
  attacker cannot read the session token). Exempting these verbs is a
  textbook footgun.
- **Rejected because**: state-mutating verbs MUST be CSRF-protected;
  RFC idempotence is orthogonal to attacker authority.

### C: Custom middleware for the comments edit/delete routes
A per-route middleware that does the same thing as the global one.

- **Pros**: Local reasoning.
- **Cons**: Duplicate code; drift risk; violates the slice-1 invariant
  that CSRF is layer-wide. New developer reading the comments handler
  has to verify CSRF is actually applied.
- **Rejected because**: zero benefit; layer-wide middleware is the
  correct shape.

## Consequences

### Positive
- Zero code change to slice-1 CSRF middleware. Slice 5 inherits the
  proven double-submit implementation verbatim.
- Uniform security posture: every state-mutating verb gets CSRF
  protection by default. New verbs added in future slices (PUT, etc.)
  inherit automatically.
- htmx `hx-delete` (empty body) is supported by the `HX-CSRF` header
  fallback, which slice 2 already wired.

### Negative
- The DELETE-with-empty-body / header-based-token detail is non-obvious
  to a developer reading the route table. This ADR is the durable
  pointer.

### Neutral
- No new dependencies. No new tests required beyond the acceptance
  scenarios that already exercise CSRF rejection (slice-1
  `@nfr-sec-04`-style scenarios cover POST; the slice-5 acceptance
  suite extends coverage to PATCH and DELETE with `@nfr-sec` tagging
  per DISTILL's discretion).

## Verification

- The slice-5 acceptance suite includes one scenario per new verb
  asserting that a request missing the CSRF token returns 403:
  - `PATCH …/comments/{id}` without `_csrf` form field → 403.
  - `DELETE …/comments/{id}` without `HX-CSRF` header → 403.
- The slice-5 acceptance suite includes one scenario per new verb
  asserting that a request with a valid CSRF token proceeds to the
  handler (200 or 302 depending on htmx vs full-page response).
- The middleware registration in `build_router` is unchanged from
  slice 1; `git diff` on `build_router` between slice-4 and slice-5
  shows zero CSRF-related lines added.
