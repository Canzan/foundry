# Non-Functional Requirements — Token-Management API

> SECURITY-heavy by nature: this exposes machine-token MANAGEMENT to a machine-token bearer
> caller — a long-lived, copy-pasteable credential. The defining NFR is the authz/escalation
> model (NFR-TMA-SEC-08). Every NFR is testable and solution-neutral (constraints + observable
> properties; DESIGN picks mechanisms). The API must uphold the SAME guarantees the web UI
> ratified in `machine-token-admin-ux/discuss/nfrs.md` (NFR-MT-SEC-01..07) — these carry over and
> are re-stated here for the JSON surface — PLUS the new escalation analysis (SEC-08).
> IDs referenced from `stories.md` and `wave-decisions.md`.

## Security

### NFR-TMA-SEC-01 — Token value never persisted, never logged (carried: NFR-MT-SEC-01)
No token VALUE is ever written to the database, a log, an error envelope, or any retrievable
store by the API surface. (In v1 there is no API mint, so no value is produced over the API at
all; this NFR governs any future programmatic mint and forbids the adapter from logging a mint
response body.)
- **Measurable**: a log scan during any token-API acceptance run finds no token-value substring;
  the `machine_tokens` table has no secret column (unchanged from the shipped table).
- **Verify**: log-scan assertion; static check that no migration adds a secret column.

### NFR-TMA-SEC-02 — LIST never exposes a token value (carried: NFR-MT-SEC-02)
The LIST response (and any per-token JSON) exposes ONLY `jti` + metadata — never a `value`,
`token`, `secret`, or `hash` field. There is no "reveal" route.
- **Measurable**: `GET .../tokens` response JSON contains no value/token/secret/hash key; no
  endpoint returns a token value on a read path.
- **Verify**: an API contract assertion over the LIST response keys; mirrors `TokenView` which has
  no value field by construction.

### NFR-TMA-SEC-03 — Refusals are non-enumerable (carried: NFR-MT-SEC-03 / NFR-MT-REL-03)
A non-management caller and a cross-workspace / unknown `jti` are refused WITHOUT revealing the
surface or token existence. A cross-workspace `jti` returns the IDENTICAL `NotFound` (404) as an
unknown `jti`; a non-management caller gets `Forbidden` (403) with no registry leak; every bearer
authentication failure returns the byte-identical `Unauthorized` (401).
- **Measurable**: cross-workspace revoke and unknown-jti revoke return identical status+body;
  non-management LIST/REVOKE returns 403 with no token data; no response is an existence oracle.
- **Verify**: adversarial acceptance (US-TMA05) — non-management, cross-workspace, unknown-jti.
  Reuses the SHIPPED non-enumerable `NotFound` in `revoke_token` and the identical 401 in
  `token_auth`.

### NFR-TMA-SEC-04 — Revocation effective on the next request (carried: NFR-MT-SEC-05)
A revoked token (`revoked_at` set, via the API) is refused on its very next `/api/v1` request via
the SHIPPED per-request `jti` denylist. No token cache keeps a revoked credential alive. This holds
for revoke-self too.
- **Measurable**: API revoke → the next call with that token (or the caller's own, for revoke-self)
  is refused (one-request latency).
- **Verify**: acceptance (US-TMA02, US-TMA03) revokes then asserts the next call is 401.

### NFR-TMA-SEC-05 — Management is attributable (carried: NFR-MT-SEC-06)
A management action over the API is attributable to the calling principal's bound `user_id`. The
LIST surfaces `created_by`/`minted_by` for each token. Any future API-minted token records
`created_by = principal.user_id()` (the bound admin), exactly as the use-case does today.
- **Measurable**: the LIST shows `minted_by`; any API mint persists a non-NULL `created_by` equal to
  the caller's bound user.
- **Verify**: row-level check + LIST contract assertion (US-TMA01).

### NFR-TMA-SEC-06 — The JSON surface inherits the SHIPPED bearer-auth contract (analog of NFR-MT-SEC-07)
The token routes are mounted on `/api/v1` OUTSIDE session+CSRF (bearer-only, CSRF-exempt by
construction — the machine request carries a JWT and NO cookie). Authentication is the SHIPPED
`MachinePrincipal` extractor (`token_auth::authenticate`): fail-closed, EdDSA-pinned, `iss`/`aud`
validated, expired/forged/revoked/unknown-jti all collapse to an identical 401. The browser
session/CSRF path is byte-for-byte unchanged.
- **Measurable**: the existing foundry-api bearer-auth scenarios stay green; token routes reject
  missing/malformed/expired/forged/revoked bearers identically (401).
- **Verify**: the SHIPPED `token_auth` unit tests + new token-route auth scenarios.

### NFR-TMA-SEC-07 — Programmatic management is rate-limited / abuse-bounded (NEW)
Programmatic management mutations (REVOKE in v1; any future MINT) are bounded against abuse loops a
human UI does not invite. A guardrail metric tracks management-mutation rate per principal; a sane
per-principal cap throttles a revoke storm (and, for any future mint, a mint loop).
- **Measurable**: a burst of management mutations beyond the cap is throttled (429 or equivalent,
  DESIGN picks); a guardrail metric exposes the per-principal mutation rate.
- **Verify**: a burst scenario asserts throttling; the guardrail metric is emitted. (Numbers +
  mechanism = DESIGN; Q-RATE-LIMIT.)

### NFR-TMA-SEC-08 — Authz / privilege-escalation model is explicit and bounded (NEW — THE CRUX)
The API must NOT create an unbounded credential self-replication surface. **v1 ratified model
(option c, Q-AUTHZ):** a machine-token bearer may LIST and REVOKE (including revoke-self), gated by
`is_workspace_admin` on the bound user; a machine-token bearer may **NOT MINT** — provisioning a new
credential is a human-session action (`/admin/tokens`) only. No mint loop is reachable from a bearer
credential. Escalation is workspace-confined (a bearer can never act on another workspace's tokens).
- **Measurable**: there is NO bearer-reachable mint route in v1; a management bearer can LIST +
  REVOKE within its workspace and nothing outside it; a non-management bearer is refused (403).
- **Verify**: a route-surface assertion (no POST mint route on `/api/v1/.../tokens` in v1); the
  walking-skeleton scenario (US-TMA01) proves authorized-vs-refused; US-TMA05 proves the
  cross-workspace + non-management boundary.
- **If the user ratifies programmatic MINT into v1 (option c+b):** mint is reachable ONLY by a
  bearer carrying an explicit `tokens:manage` capability claim (never a plain admin-bound token);
  a management token MUST NOT be able to mint another management token (no self-replication of the
  capability); mint is rate-capped (SEC-07). This sub-requirement activates only on that ratification.

## Reliability / Correctness

### NFR-TMA-REL-01 — Revoke is idempotent over the API (carried: NFR-MT-REL-02)
Revoking an already-revoked token via the API is a no-op success; concurrent revokes converge to
Revoked. Re-DELETE of a revoked `jti` succeeds.
- **Verify**: double-revoke + concurrent-revoke acceptance (US-TMA02). Reuses the idempotent
  `revoked_at` re-stamp in `revoke_token`.

### NFR-TMA-REL-02 — Reads + mutations are workspace-isolated (carried: NFR-MT-REL-03)
`list_tokens` returns only the acting workspace's rows; `revoke_token` acts only on the acting
workspace's `jti` (else non-enumerable `NotFound`). No cross-workspace read or mutation is possible.
- **Verify**: cross-workspace acceptance (US-TMA05). Reused unchanged from the use-cases.

### NFR-TMA-REL-03 — The token routes do not regress the shipped /api/v1 surface
Adding the token routes leaves the existing issue/comment routes, the bearer extractor, and the
session/CSRF browser path byte-for-byte unchanged.
- **Verify**: the full `foundry-acceptance` suite green before stays green after.

## Performance

### NFR-TMA-PERF-01 — List and revoke are interactive (carried: NFR-MT-PERF-01)
A LIST and a REVOKE over the API complete within an interactive budget consistent with the existing
web/API tier (target ≤200 ms server-side, excluding network), with NO regression to the SHIPPED
per-request verify path.
- **Measurable**: server-side LIST/REVOKE handler time ≤200 ms p95 in the acceptance harness;
  verify-path latency unchanged.
- **Verify**: timing assertions in the LIST/REVOKE scenarios; existing verify-path benchmarks within
  budget.

## Contract / Interoperability

### NFR-TMA-CON-01 — Stable JSON error envelope across all token routes (mt-api-job-3)
Every non-2xx token-route response is the SHIPPED `ErrorBody {error:{code,message}}` with the
conventional status from `status_for`: 401 unauthorized, 403 forbidden, 404 not_found, 422 +
specific code for validation. The envelope never carries HTML, SQL, a stack trace, or credential
material.
- **Measurable**: each refusal/validation across the token routes matches the shipped envelope
  shape + status; an integrator can branch on `error.code` reliably.
- **Verify**: contract assertions per route (US-TMA04); reuses `status_for` unchanged.

### NFR-TMA-CON-02 — Returned representation equals a subsequent read
Any token representation the API returns (LIST rows; a future mint's metadata) equals what a
subsequent LIST returns for the same token (modulo `revoked`/`last_used_at` changing over time).
- **Verify**: a read-after-write contract assertion on the LIST shape.

## Data Integrity

### NFR-TMA-DATA-01 — No secret column ever added (carried: NFR-MT-DATA-02)
No migration in this feature adds a token/secret/hash column to `machine_tokens`. (This feature
adds NO migration at all under the recommended scope — it is a pure adapter feature; this NFR
guards against drift if any is proposed.)
- **Verify**: static review of the feature's migrations (expected: none).

## Invariants (carried, must not regress)

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
- The SHIPPED verify path + per-request `jti` denylist + `iss`/`aud`/EdDSA pinning are unchanged;
  this feature adds a MANAGEMENT surface around them, not new verify logic.
- The use-cases `foundry_services::tokens::{mint_token,list_tokens,revoke_token}` are REUSED AS-IS
  (mutation-hardened 100%), not reimplemented; the foundry-api `MachinePrincipal` extractor and
  `status_for` envelope are reused unchanged.
- foundry-api does NOT name `foundry_store::Store` (the boundary-guard `foundry-api ⊀ foundry-store`
  ban) — it reaches the use-cases through the `Services` handle, exactly as the issue/comment
  routes do.
- The `foundry-acceptance` suite green before this feature stays green after.
