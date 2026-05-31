# Programmatic Foundry (Feature A) — Error Mapping & Observability

Owner: solution-architect (Morgan). Scope: API-tier error handling + metrics/tracing for Feature A.
MIRRORS `docs/feature/foundry-backend-mvp/design/error-and-observability.md`. Reuses the existing
`tracing` (structured JSON to stdout) + `metrics`/`metrics-exporter-prometheus` stack
(`crates/foundry-app/src/main.rs`, `metrics_server.rs`) — Feature A adds NO new observability
dependency or sidecar.

## Error model

One `ServiceError` enum (in the `foundry-services` crate, ADR-W07) is the single source of truth for
use-case failures; the `foundry-api` adapter maps it to a status + JSON envelope, and (Feature B) the
HTML adapter maps the SAME enum to an HTML fragment. This is the error-side proof of rule-parity: API
and UI surface the *same* error from the *same* variant (NFR-WEB-API-CON-02).

```text
// crates/foundry-services/src/error.rs (illustrative)
enum ServiceError {
    NotFound,                         // 404 / not_found
    Forbidden,                        // 403 / forbidden        (authz: not member / not author / out of scope)
    Unauthorized,                     // 401 / unauthorized     (no valid principal — see auth errors below)
    Validation { code, message },     // 422 / <code>           (e.g. title_required, "Title is required")
    Gone,                             // 410 / gone             (soft-deleted comment)
    Conflict,                         // 409 / conflict         (duplicate, if surfaced)
    Internal(anyhow::Error),          // 500 / internal         (logged with detail; envelope is generic)
}
```

The **JWT/denylist authentication failures are decided in the `foundry-api::token_auth` extractor
BEFORE any service is called** (the extractor is the authentication boundary; the service only sees
an already-authenticated `Principal`). They all map to `401 unauthorized` with the generic envelope
(no detail that distinguishes *why* a credential failed — non-enumerable, mirroring the sign-in
posture), while the *reason* is logged + counted (see Metrics). Scope/membership failures, by
contrast, are decided in the service and surface as `403 forbidden`:

| Auth-tier condition | Decided in | HTTP | envelope `code` |
|---|---|---|---|
| Missing/malformed `Authorization` | extractor | 401 | `unauthorized` |
| Bad EdDSA signature | extractor | 401 | `unauthorized` |
| Wrong `alg` / `alg:none` (alg-confusion) | extractor | 401 | `unauthorized` |
| Expired (`exp`) | extractor | 401 | `unauthorized` |
| `jti` absent from registry (forged/withdrawn) | extractor | 401 | `unauthorized` |
| `jti` present but `revoked_at IS NOT NULL` (revoked) | extractor | 401 | `unauthorized` |
| Valid token, out-of-scope team / not a member | service (`Forbidden`) | 403 | `forbidden` |

### API mapping (api-contract.md restated for the adapter)

| `ServiceError` | HTTP | envelope `code` | envelope `message` |
|---|---|---|---|
| `Unauthorized` | 401 | `unauthorized` | "Authentication required" (covers every JWT failure: missing/malformed/bad-signature/wrong-alg/expired/forged-jti/revoked — non-enumerable; the specific reason is logged + counted, never returned) |
| `Forbidden` | 403 | `forbidden` | "You do not have access to this resource" (or the UI-equivalent copy where one exists) |
| `NotFound` | 404 | `not_found` | "Not found" |
| `Validation{code,message}` | 422 | `code` | `message` (the SAME copy the UI shows, carried on the variant) |
| `Gone` | 410 | `gone` | "This resource has been deleted" |
| `Conflict` | 409 | `conflict` | resource-specific |
| `Internal` | 500 | `internal` | "Internal error" (detail is logged, never returned) |
| (axum body-parse failure) | 400 | `invalid_json` | "Request body is not valid JSON" |

The mapping lives in one `impl IntoResponse for ApiError` in the `foundry-api` crate so there is
exactly one place a status/code is decided. **Every** branch produces a JSON envelope — there is no
HTML error path in the api tier (NFR-WEB-API-CON-03; enforced by the boundary guard,
`boundary-guard.md`). No `ServiceError::Internal` ever leaks SQL, a stack trace, the JWT, or any key
material into the envelope (NFR-WEB-API-SEC-03).

## Tracing

Reuse the existing `tracing` JSON-to-stdout setup (`main.rs:init_tracing`). Feature A adds:
- A span field `api.jti` (UUID) and `api.workspace_id` on authenticated `/api/v1` requests —
  **never** the JWT or the signing key (NFR-WEB-API-SEC-03; the JWT is `SecretString`, un-loggable
  by construction).
- Auth-failure events log the *reason* (`bad_signature`/`wrong_alg`/`expired`/`forged`/`revoked`/
  `out_of_scope`) and, only once the signature verified and a registry row was consulted, `jti`; a
  malformed/bad-signature token logs no identifying value.
- The existing per-request metrics layer (`metrics_server::request_tracking_layer`, applied
  uniformly in `build_router`) already emits `http_requests_total{path,method,status}` + a duration
  histogram for **every** routed request — so `/api/v1` endpoints are covered with **zero new
  instrumentation**. (Cardinality note: the existing layer keys on the matched route *pattern*, not
  the raw path, so `/api/v1/teams/{team}/projects/{project}/issues` is one series, not one per
  team/project — consistent with the slice-6 ADR-011 bounded-cardinality invariant.)

## Metrics

No new metric is strictly required for Feature A (the existing request counter/histogram covers the
new routes). Two **optional, bounded** additions are recommended for operating the new credential
surface, following the repo's register-at-0 + bounded-label discipline (`main.rs` PROBE_NAMES /
slice-8 gauges):

- `machine_token_auth_failures_total{reason}` — counter, `reason ∈ {missing, malformed,
  bad_signature, wrong_alg, expired, forged, revoked, out_of_scope}` (a **closed, code-defined**
  set, like `PROBE_NAMES`; never request-derived; extended from the prior set with the JWT-specific
  `bad_signature`/`wrong_alg`/`forged`). Register-at-0 for all reasons so dashboards show the full
  "no auth failures" baseline. `wrong_alg` spiking is a strong alg-confusion-attack signal; `forged`
  spiking signals replayed/withdrawn tokens.
- `machine_tokens_active` — gauge, count of non-revoked, non-expired tokens; folded into the
  existing 5 s pool-poll loop (`main.rs:249`), same precedent as `bootstrap_tokens_unclaimed`. No
  new task.

Both are unlabelled-or-bounded and emit from existing loops — they honor the cardinality invariant
and add no new collection machinery. If the user prefers the absolute minimum, Feature A ships with
*only* the inherited request metrics and adds these in a follow-up; flagged as a non-blocking
ratification item.

## Startup probe (Earned Trust — extends, not adds)

The existing `store` startup probe (`Store::probe`, invoked via `record_probe_result("store", …)`
in `main.rs:456`) is extended to assert the `machine_tokens` table + its `jti`, `revoked_at`,
`scope_team_id`, `expires_at`, `workspace_id`, `user_id` columns exist in `current_schema()` (see
`architecture.md` §Earned Trust). A binary booting against a pre-0007 schema emits
`health.startup.refused {probe:'store', reason:'machine_tokens missing columns'}`, increments the
existing `probe_failures_total{probe_name="store"}` counter, and exits non-zero — the exact posture
already wired. This rides inside the `store` probe (no change to that probe name).

**NEW — Ed25519 key-material probe** (the JWT override's added substrate assumption): at composition
the `MachineTokenVerifier` (and `MachineTokenSigner`, on an issuing binary) is built by parsing the
configured key material, then `self_test` signs + verifies a throwaway claim to prove the keypair
round-trips in THIS environment. A malformed/mismatched key emits `health.startup.refused
{probe:'machine_token_key', reason:'…'}` and exits non-zero — wire-then-probe-then-use, so the binary
never reaches production silently 401-ing every API request. This is a NEW entry in `PROBE_NAMES`
(`machine_token_key`), registered-at-0 like the others.

## What stays unchanged

- The browser-path error rendering, the `/healthz`/`/readyz` handlers, the metrics sidecar, the
  graceful-shutdown drain, the GC/outbox poll loops — all untouched. Feature A adds an error mapping
  in the api adapter and two optional bounded metrics; it edits no existing observability behavior
  (NFR-WEB-COMPAT-01).
