# ADR-001 — Request → workspace resolution mechanism

## Status
Proposed (DESIGN, Propose mode; user ratifies at the post-roadmap checkpoint).

## Context
Today exactly one workspace exists, so "the workspace" is implicit. With many workspaces, every
request needs a single, auditable answer to "which workspace does this request act on?"
(DM1, NFR-MWT-SEC-03/06). The answer must yield EXACTLY one `acting_workspace_id` and fail closed
if none/ambiguous. Two surfaces have different natural sources:

- **Web htmx tier** — a human session. The shipped code ALREADY carries
  `SessionUser{ user_id, workspace_id }` in the session (`signin.rs:140`, `session.rs`), where
  `workspace_id` is `first_workspace()` (the sole workspace). This is *already* a session-carried
  active-workspace claim — it just resolves to the only workspace.
- **JSON `/api/v1`** — a machine-token bearer. The shipped `MachinePrincipal` extractor ALREADY
  produces `Principal::Machine{ workspace_id, … }` from the token's claim
  (`foundry-api/src/lib.rs:583`). `token.workspace_id` is already the authoritative acting workspace.

## Options considered
- **(a) Session-carried active workspace (web) + token.workspace_id claim (API) — hybrid.** EXTEND
  the two seams that already exist. Web: at sign-in/switch, set the session's active workspace from
  the user's *memberships*; per request, read it and verify membership. API: keep `token.workspace_id`.
- **(b) URL/path segment** (`/w/{slug}/...`). Resolution from the path. Pro: bookmarkable,
  multi-tab. Con: rewrites every shipped route + every template link; a path-supplied id is a
  *client-supplied* workspace — exactly the trust footgun NFR-MWT-SEC-06 warns against; would touch
  far more shipped surface than dropping a guard.
- **(c) Host/subdomain** (`acme.foundry...`). Resolution from the Host header. Pro: clean isolation
  feel. Con: needs DNS/TLS per tenant, contradicts the single-binary/no-CDN constraint, heavy ops.
- **(d) A single global resolution middleware** that re-derives the workspace for every request.
  Pro: one place. Con: re-deriving per request is what the session claim already memoizes; adds a
  layer with no benefit over (a).

## Decision
**(a) Hybrid.** EXTEND the shipped seams:
- **Web**: the session's `workspace_id` becomes the **active** workspace, set at sign-in (ADR-005)
  and re-stamped by the switcher. Per request, `SessionUser` resolution verifies the user is still a
  member of that workspace; if not (membership revoked, none selected), it **fails closed** — the
  request is refused, never defaulted. The acting workspace is NEVER taken from a client-supplied
  field/path/query.
- **API**: `token.workspace_id` (already shipped) is the acting workspace. Session-authenticated
  `/api/v1` calls (if any) resolve like the web tier.

Handlers consume a thin resolved value (`ActingWorkspace` newtype, ADR-002), not a raw `Uuid`
parsed from the request, so "the handler trusts the resolved seam" is the only typed path.

## Consequences
- **Positive**: smallest possible change (the seam exists); no route/template rewrite; no DNS/TLS;
  honors single-binary + no-CDN; the acting workspace is provably not client-supplied; resolution is
  a cheap session/claim read (NFR-MWT-PERF-02). Single-workspace installs keep working unchanged.
- **Negative**: no bookmarkable per-workspace URLs in v1 (a contractor switches via the switcher,
  not by editing the URL); multi-tab "two workspaces at once" is not supported (the session has one
  active workspace) — acceptable for v1, revisit with path-scoping later if demanded.
- **Trade-off point**: usability (multi-tab) traded for security simplicity + minimal blast radius.

## Slice alignment
Walking skeleton (Slice 1) ships the resolution seam first; Slice 3 proves the API leg; Slice 5
relies on it defaulting the one workspace for upgraded installs.
