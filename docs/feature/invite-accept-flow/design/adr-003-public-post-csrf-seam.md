# ADR-003 — CSRF on the public (signed-out) accept POST

## Status
Proposed (DESIGN wave). Resolves the public-POST CSRF open decision.

## Context
NFR-6: the accept POST is state-changing (it consumes an invite + writes a credential) but the caller
is **signed-out** — there is no session yet. The shipped `csrf_middleware` (`csrf.rs:96-173`) is a
**double-submit** check: it compares a `_csrf` form field (or `x-csrf-token` header) against a
`csrf` cookie via constant-time equality. Crucially, double-submit needs only a cookie + a matching
token — it does NOT require a session. The middleware exempts only safe methods and the single path
`/bootstrap` (`is_exempt_path`, `csrf.rs:61-67`), whose pre-session protection comes from a single-use
URL token instead.

The sign-in flow already faces this exact "signed-out form that needs CSRF" problem and solves it:
`ensure_csrf_cookie` (`signin.rs:287-299`) reads an existing `csrf` cookie or mints a fresh one on the
signed-out GET, and the rendered form carries the matching hidden `_csrf`.

## Options considered
- **(a) Mint the CSRF cookie on the GET accept page; POST mounts under the shipped middleware
  (RECOMMENDED).** Reuse `ensure_csrf_cookie` verbatim — the GET sets the cookie + hidden field, the
  POST's double-submit check works with no session, exactly as sign-in does.
- **(b) Add `/invites/accept` to `is_exempt_path` (like `/bootstrap`).** REJECTED — the `/bootstrap`
  exemption is justified by its single-use URL token providing CSRF-equivalent protection. The accept
  POST's `sig` is NOT single-use (it is re-openable until consumed) and the GET→POST CSRF cookie costs
  nothing, so there is no reason to weaken to an exemption. Keeping real double-submit is strictly safer.
- **(c) A bespoke per-invite CSRF token bound to the invite_id.** REJECTED — reinvents the shipped
  double-submit for no gain; more code, more surface, no added protection over (a).

## Decision
**(a)** — the GET `show_accept_form` calls `ensure_csrf_cookie` (reused) to set/read the `csrf` cookie
and renders the matching hidden `_csrf` field. The POST `submit_accept` mounts UNDER the shipped
`csrf_middleware` on the public layer of `build_router`. **NO** new middleware, **NO** `is_exempt_path`
entry. A POST with a missing/mismatched `_csrf` is refused (403) by the shipped middleware before the
handler runs — no consume, no password write (NFR-6, AC-02.8).

## Consequences
- **Positive**: zero new CSRF code; identical to the proven signed-out sign-in pattern; the POST is
  genuinely double-submit-protected despite being public.
- **Negative**: the GET must always emit the cookie (a `Set-Cookie` on first render) — negligible.
- **Security**: a cross-site forged POST lacks the matching cookie+token pair and is refused; the
  state-changing credential path is CSRF-hardened even though the user is signed-out.

## Relationship
Reuses `ensure_csrf_cookie` (`signin.rs`) and `csrf_middleware` (`csrf.rs`) verbatim. Does NOT touch
`is_exempt_path`. Mounts on the public layer per `build_router` (D? in wave-decisions: route on the
public layer, NOT the instance-admin gate).
