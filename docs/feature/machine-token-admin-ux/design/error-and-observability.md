# Error Mapping and Observability

DESIGN (Propose) for how `ServiceError` maps to the admin surface, the metrics, and the
no-secret-in-logs rule. Mirrors the Feature-A `error-and-observability.md` discipline (one
`ServiceError` → one HTTP/HTML mapping). Decisions: `wave-decisions.md`.

## Error mapping (`ServiceError` → admin web surface)

The token-admin use-cases return the SHARED `ServiceError` enum (`foundry-services/src/lib.rs:187`).
The `admin_tokens` handlers map each variant to a browser response, matching how `projects.rs` /
`comments.rs` map theirs (full page vs htmx `ErrorFragment`):

| Condition | `ServiceError` | HTTP | Web rendering |
|---|---|---|---|
| Non-admin / not signed in | `Forbidden` (use-case) / no session (handler) | **404 generic** (non-enumerable) / redirect `/sign-in` | generic "Not found" page — never confirms the surface exists (US-MT05, NFR-MT-SEC-03) |
| Cross-workspace `jti` on revoke; unknown `jti` | `NotFound` | **404 generic** | same generic not-found — non-enumerable (US-MT03 scenario 3) |
| TTL missing / over cap | `Validation { code:"ttl_required" / "ttl_over_cap" }` | **422** | mint form re-render (full) or `ErrorFragment` (htmx) with "Maximum expiry is 365 days" (US-MT04 scenario 2) |
| Scope team not in workspace | `Validation { code:"scope_team_not_in_workspace" }` | **422** | mint form re-render / fragment — no token issued (US-MT04 scenario 3) |
| Label empty / too long | `Validation { code:"label_invalid" }` | **422** | mint form re-render / fragment |
| Issuing not enabled (signer absent) | (handler short-circuit, not a `ServiceError`) | **403** | "Issuing tokens is not enabled on this server" — never a 500, never a partial token (US-MT01 scenario 3, signer.md) |
| Mint sign/persist failure | `Internal` | **500** | clean 500 page (full) or 500 `ErrorFragment` (htmx) — **never a partial token** (NFR-MT-REL-01) |
| Revoke already-revoked | `Ok(())` | **303 → list** | row shows Revoked; idempotent success (NFR-MT-REL-02) |

The non-enumerable refusal SHAPE: `Forbidden` from the use-case is rendered by the handler as the
SAME generic 404 page as `NotFound`, so a non-admin cannot distinguish "you're not an admin" from
"this doesn't exist" (NFR-MT-SEC-03). This is the deliberate divergence from `comments.rs` (which
returns an explanatory 403) — the token surface is more sensitive and must not leak its existence.

## All-or-nothing mint render (NFR-MT-REL-01)

The mint success path renders `TokenMintedPage` to a COMPLETE `String` BEFORE any bytes hit the
response — the SAME guarantee `render_board` gives (`projects.rs:521-545`). A render `Err` maps
centrally to a clean 500 (full page) or a 500 fragment (htmx) via the `render_500` helper
(`projects.rs:554`), so the client can NEVER see a half-emitted page carrying a partial token. The
crafter SHOULD add a `force_token_render_failure` test seam mirroring `force_board_render_failure`
(`foundry-app/src/lib.rs:113`) so the all-or-nothing mapping is observable in acceptance without a
genuinely broken template (NFR-MT-REL-01 verify, "mirrors the existing force_board_render_failure
test seam pattern").

## Metrics

Reuse the existing `metrics` recorder + the `http_requests_total{path,method,status}` layer
(`foundry-app/src/lib.rs:328`) — the admin routes are instrumented for free by that layer. Add two
feature-specific counters (cheap, bounded-cardinality, NO secret labels):

| Metric | Type | Labels | Incremented |
|---|---|---|---|
| `machine_tokens_minted_total` | counter | `scope` ∈ {`workspace`,`team`} | on a successful `mint_token` |
| `machine_tokens_revoked_total` | counter | (none) | on a successful `revoke_token` (including idempotent re-revoke) |

Deliberately NO label carries `jti`, `label`, `user_id`, or any token material (unbounded
cardinality AND a leak vector). The existing `health.startup.refused {probe:'machine_token'}` event
already covers the signer-key boot failure (signer.md); no new startup metric is needed.

Optional (recommend, not required): observe mint/revoke handler latency against the existing
duration histogram to back NFR-MT-PERF-01 (≤200ms p95) — already emitted by the request-tracking
layer per path, so it is free.

## The no-secret-in-logs rule (NFR-MT-SEC-01)

Non-negotiable, enforced by construction:
- The signing key is a `SecretString` (no `Debug`/`Display`); never logged. `AppState`'s `Debug`
  omits it (signer.md).
- The minted token is a `SecretString` from `mint` (`foundry-auth:124`); never logged, never put in
  an error message, never in a metric label. The `MintedToken.value` field is the only place it
  lives, and it drops after one render (DD7).
- Error logs use the existing `tracing::error!(error = %err, …)` shape (`projects.rs:351`) where
  `err` is a `ServiceError`/`StoreError`/`askama::Error` — NONE of which carry token material.
- Acceptance asserts (per NFR-MT-SEC-01 verify): a log scan during an issuance run finds no token
  value substring; a DB scan finds no secret column (NFR-MT-DATA-02). DESIGN adds nothing that
  could carry the value into either sink.

## Verifier-only graceful path (NFR-MT-SEC-04)

A verifier-only binary (`machine_token_signer == None`) renders the "issuing not enabled" notice on
`GET /admin/tokens` (list still works) and returns a clean 403 on `POST /admin/tokens` — observable
as a normal 403 in the metrics layer, never a 500, never a probe failure (the binary booted fine; it
simply has no issuer capability). US-MT00 scenario 2: "the server does not error."
