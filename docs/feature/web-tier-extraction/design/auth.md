# Programmatic Foundry (Feature A) — Machine-Token Authentication Design

Owner: solution-architect (Morgan). Scope: US-W05b — a first-class, additive, admin-governable
machine credential. This MIRRORS `docs/feature/foundry-backend-mvp/design/auth.md` and treats the
token surface with the same password-model rigor (DISCUSS risk register: "getting it wrong is a
credential-leak risk"). **The token mechanism was RATIFIED 2026-05-31 as JWT/Ed25519, OVERRIDING the
proposed opaque-token recommendation.** This document is rewritten to the ratified mechanism.

Hard invariants (NFR-WEB-API-SEC-01..03, D5, out-of-scope.md):
- **Additive only**: the browser session/cookie/CSRF/argon2/brute-force path is byte-for-byte
  unchanged. A machine-token request needs no session cookie and no CSRF token.
- **Revocable + scope-bounded**: an admin issues and revokes; a revoked token is refused on its next
  use; a token cannot exceed the authorization of the principal it is bound to (no escalation).
- **Secrets handled like credentials**: the signed JWT is never persisted, never logged, transmitted
  only over the existing transport posture.

## Ratified mechanism — bearer JWT signed with Ed25519 (EdDSA) + a Postgres `jti` denylist

> The proposed recommendation (Option A — opaque random `fdy_…`, SHA-256 at rest, per-request
> lookup, zero new deps) was **superseded by user ratification 2026-05-31**. The user chose a
> standards-based, asymmetric, self-describing credential. The original option table is preserved in
> `wave-decisions.md` ADR-W02 for the record. This section designs the ratified choice in full.

A machine token is a **compact JWS (JWT) signed with Ed25519 (algorithm `EdDSA`)**. The signed token
IS the secret; the server stores no token secret or hash. Revocation — a HARD requirement
(NFR-WEB-API-SEC-02, US-W05b scenario 2) that a stateless JWT cannot satisfy alone — is restored by a
**`jti` denylist/registry** in Postgres, checked on every request.

### Claims (the v1 token contract)

```text
{
  "sub":   "<user_id UUID>",        // the bound principal; authorization is computed from this
  "scope": "<team_id UUID | null>", // team-narrowing filter; null = workspace-wide (still bounded by sub's membership)
  "iat":   <unix>,                  // issued-at
  "exp":   <unix>,                  // expiry — short-ish; default 90 days, admin-settable
  "jti":   "<UUID>"                 // unique token id; the denylist/registry key
}
```

`iss`/`aud` are fixed constants for the single-issuer Feature-A case and **validated**. They are pinned
to `iss = "foundry"` (`foundry_auth::MACHINE_TOKEN_ISS`) and `aud = "foundry-api"`
(`foundry_auth::MACHINE_TOKEN_AUD`): `MachineTokenSigner::mint` always stamps these exact values, and
`MachineTokenVerifier` enforces them via `Validation::set_issuer`/`set_audience` — a validly-signed
token carrying any other `iss` or `aud` is refused (401) before any registry lookup. `nbf` (not-before)
is likewise validated (`validate_nbf = true`), so a not-yet-valid token is refused. The `workspace_id`
is resolved from `sub` at the store (the user's workspace), so it is not a trusted claim.

### Per-request verification (fail-closed, in `foundry-api::token_auth`)

1. Parse `Authorization: Bearer <jwt>`; missing/malformed → **401**.
2. **Verify the EdDSA signature** against the configured Ed25519 **public** key, with the algorithm
   **allow-list pinned to exactly `[EdDSA]`**. Any other `alg` (RS256, HS256, …) or `alg: none` →
   **401**. *This closes the alg-confusion / `none` footgun — the single most important JWT pitfall.*
   Bad signature → **401**.
3. Validate `exp` (+ `iat`/`nbf` with small leeway) via the library's built-in `Validation`; expired
   → **401**.
4. **`jti` denylist check**: `find_machine_token_by_jti(jti)`. Refuse **401** if the row is absent
   (forged or withdrawn), `revoked_at IS NOT NULL` (revoked), or the registry row's `expires_at`
   has passed. Only a live row proceeds.
5. Build `Principal::Machine { user_id: sub, workspace_id, jti, scope }` and hand it to
   `foundry-services`; best-effort, non-blocking `touch_machine_token_last_used(jti)`.

Steps 2-3 are pure crypto (no DB); step 4 is the one indexed PK lookup (~1 ms — the same operational
shape as the existing session lookup, NFR-WEB-PERF-02). The JWT does not remove the DB round-trip;
it adds a signature verify on top, which is the cost of standards-based revocable tokens.

### The `MachineToken` primitive (`foundry-auth`, CREATE NEW)

`foundry-auth` is the natural home (it already owns argon2/HMAC/bootstrap) and gains the new
`jsonwebtoken` dependency:

```text
// crates/foundry-auth/src/machine_token.rs  (illustrative shape, NOT implementation)
pub struct MachineTokenClaims { pub sub: Uuid, pub scope: Option<Uuid>, pub iat: i64, pub exp: i64, pub jti: Uuid }

pub struct MachineTokenSigner { /* holds the Ed25519 EncodingKey (private), SecretString-wrapped */ }
impl MachineTokenSigner {
    pub fn from_pkcs8_env(secret: &SecretString) -> Result<Self, AuthError>;   // parse key from config
    pub fn mint(&self, claims: &MachineTokenClaims) -> Result<SecretString, AuthError>; // EdDSA-sign → compact JWT
}

pub struct MachineTokenVerifier { /* holds Ed25519 DecodingKey(s) (public) + EdDSA-pinned Validation */ }
impl MachineTokenVerifier {
    pub fn from_public_keys_env(keys: &[String]) -> Result<Self, AuthError>;   // parse one-or-more public keys
    pub fn verify(&self, jwt: &str) -> Result<MachineTokenClaims, AuthError>;  // alg=EdDSA pinned; validates exp/iss/aud
    pub fn self_test(&self, signer: &MachineTokenSigner) -> Result<(), AuthError>; // Earned-Trust round-trip probe
}
```

The minted JWT is wrapped in `SecretString` (as `BootstrapToken.raw` is) so it cannot be
`Debug`/`Display`-logged. The **JWT is shown to the admin exactly once at issuance** and never again
— same posture as the bootstrap URL.

## Key management (NEW sub-decision — flagged for ratification in `wave-decisions.md`)

The JWT override introduces signing-key management, grounded in Foundry's existing config model:
`main.rs` reads everything from env via `std::env::var` + `dotenvy::dotenv()`; `SESSION_SECRET`
(required, ≥32 bytes, `SecretString` in `AppState`, `main.rs:151,178`) is the direct precedent.

- **Where the keypair lives — env/config (RECOMMENDED), like `SESSION_SECRET`; NOT Postgres, NOT a
  committed file.**
  - `MACHINE_TOKEN_SIGNING_KEY` — Ed25519 private key (PKCS#8 PEM, base64), `SecretString`. Required
    only on a binary configured to ISSUE tokens.
  - `MACHINE_TOKEN_PUBLIC_KEYS` — one or more Ed25519 public keys (comma-separated, newest first).
    Required on every binary that VERIFIES. A single-binary deployment sets both.
  - At boot, `AppState` gains `machine_token_verifier: Arc<MachineTokenVerifier>` (always) and, on an
    issuing binary, a `MachineTokenSigner` for the admin issue route — built exactly where
    `session_secret` is built.
- **Why config, not Postgres**: putting the private key in the DB co-locates the signing secret with
  the data it protects and complicates the boot/probe story; env-var secrets match the existing
  posture and keep the key out of the data plane. **Why not a file**: env is how every other secret
  (DB creds, `SESSION_SECRET`) already arrives; a file adds a second delivery mechanism.
- **Rotation** (issue-new-key + overlap): the verifier accepts the SMALL SET in
  `MACHINE_TOKEN_PUBLIC_KEYS` (≤2 in practice), so a new signing key rolls in while tokens signed by
  the prior key still verify until their `exp`; the old public key is dropped after the longest
  outstanding `exp`. No `kid` header is required for the single-issuer case (the verifier tries each
  configured public key — cheap). Alternative considered: a single key with a hard cutover (simpler
  config, but invalidates every outstanding token at rotation) — rejected as operationally hostile.
- **Earned-Trust key probe** (wire-then-probe-then-use): at composition the verifier (and signer, if
  present) is built by PARSING the configured key material, then `self_test` **signs a throwaway
  claim and verifies it** — proving the keypair round-trips in THIS environment. A malformed or
  mismatched key makes the binary **refuse to start** with `health.startup.refused
  {reason:'machine_token_key', detail:'…'}` and non-zero exit — never a silent "auth always 401 in
  production." See `architecture.md` §Earned Trust.

## Storage (NFR-WEB-API-SEC-03) — registry + jti denylist, see ADR-W05

New forward-only migration `crates/foundry-store/migrations/0007_machine_tokens.sql` (advisory-locked
like every migration; forward-only per the repo's ADR-003 discipline — never edit an applied file).
**The table stores no token secret and no hash** — only issuance/lifecycle metadata keyed by `jti`:

```sql
-- 0007_machine_tokens.sql — Feature A (US-W05b). Forward-only. Registry + jti denylist.
CREATE TABLE machine_tokens (
    jti           UUID PRIMARY KEY,                        -- the JWT 'jti' claim; per-request denylist lookup key
    workspace_id  UUID NOT NULL REFERENCES workspaces(id),
    user_id       UUID NOT NULL REFERENCES users(id),      -- the 'sub' / principal the token acts as
    name          TEXT NOT NULL,                           -- admin-facing label ("Devansh dashboard")
    scope_team_id UUID NULL REFERENCES teams(id),          -- NULL = workspace-wide (still bounded by user_id membership)
    created_by    UUID NOT NULL REFERENCES users(id),      -- the issuing admin (audit)
    issued_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at  TIMESTAMPTZ NULL,
    expires_at    TIMESTAMPTZ NOT NULL,                     -- mirrors the JWT 'exp'
    revoked_at    TIMESTAMPTZ NULL
);
-- No token_hash column: the JWT is the secret and is never persisted.
-- The jti PRIMARY KEY indexes the per-request lookup; no separate hash index.
CREATE INDEX idx_machine_tokens_workspace ON machine_tokens (workspace_id);
```

`Store` gains a small repository (CREATE NEW — no existing alternative): `insert_machine_token` (on
issue), `find_machine_token_by_jti` (returns workspace_id + user_id + scope + revoked/expiry, used by
the extractor), `revoke_machine_token`, `list_machine_tokens(workspace_id)`,
`touch_machine_token_last_used` (best-effort, async, non-blocking).

> **`created_by` audit column — deferred to the issuance feature.** The illustrative DDL above shows a
> `created_by UUID NOT NULL REFERENCES users(id)` audit column. Feature A intentionally does **not**
> build the admin token-issuance UX (`POST /admin/machine-tokens`); in Feature A tokens are minted in
> tests and by the operator via env/config, so there is **no issuer call-site** that could populate a
> `NOT NULL created_by`. Per ADR-003 (forward-only migrations — never edit an applied file), the
> shipped `0007_machine_tokens.sql` therefore **omits** the `created_by` column rather than adding an
> unpopulated `NOT NULL` one. The column is **deferred until the token-issuance surface lands** (§Issuance
> & revocation UX), which will introduce a forward-only follow-up migration that adds `created_by`
> populated from the issuing admin. This keeps the design and the schema honest: a column exists only
> once a writer for it exists.

Earned-Trust: the existing `Store::probe()` is extended to assert this table + its `jti`,
`revoked_at`, `scope_team_id`, `expires_at`, `workspace_id`, `user_id` columns exist in
`current_schema()` (see `architecture.md` §Earned Trust) so a binary booting against a pre-0007 DB
**refuses to start** rather than failing on first token auth.

## Authentication flow (the extractor)

The machine-token path lives in the `foundry-api` crate and is mounted so the **session and CSRF
tower layers do not run on it** — they are scoped to the cookie path (see "Coexistence" below). A
dedicated axum `FromRequestParts` extractor in `foundry-api::token_auth` authenticates `/api/v1`
requests:

```mermaid
sequenceDiagram
    autonumber
    participant Ag as Agent (script)
    participant Ex as foundry-api::token_auth extractor
    participant VK as MachineTokenVerifier (Ed25519 public key)
    participant ST as foundry-store (machine_tokens jti registry)
    participant SV as foundry-services use-case
    participant PG as Postgres

    Ag->>Ex: GET /api/v1/.../issues  (Authorization: Bearer <JWT>)
    Ex->>Ex: parse "Bearer <jwt>"; reject 401 if missing/malformed
    Ex->>VK: verify(jwt) — EdDSA signature, alg pinned [EdDSA], validate exp/iss/aud
    alt bad signature OR wrong alg / alg:none OR expired
        Ex-->>Ag: 401 unauthorized (error envelope, no data)
    else signature + exp OK → claims {sub, scope, jti}
        Ex->>ST: find_machine_token_by_jti(jti)
        ST->>PG: SELECT revoked_at, expires_at, ... WHERE jti=$1
        alt no row OR revoked_at IS NOT NULL OR expires_at < now()
            Ex-->>Ag: 401 unauthorized (forged/withdrawn/revoked/expired; no data)
        else live registry row
            Ex->>Ex: build Principal::Machine{ user_id: sub, workspace_id, jti, scope }
            Ex-)ST: touch_machine_token_last_used(jti)   %% best-effort, non-blocking
            Ex->>SV: hand Principal to the use-case
            SV->>ST: is_team_member / is_workspace_admin (SAME authz as a human)
            alt scope or membership fails
                SV-->>Ag: 403 forbidden (no change persisted)
            else authorized
                SV->>PG: read/write via existing repository + outbox
                SV-->>Ag: 200/201 JSON
            end
        end
    end
```

Key properties:
- **Fail-closed**: missing → 401; malformed → 401; bad signature → 401; **wrong `alg`/`alg:none` →
  401**; expired → 401; unknown/withdrawn `jti` → 401; revoked → 401; valid-but-out-of-scope/
  not-a-member → 403. No path reaches a use-case unauthenticated (US-W05b scenarios 2-5). This is
  the behavioral gold-test catalogued in `boundary-guard.md`.
- **Alg-confusion is closed by design**: the verifier's `Validation` allow-lists exactly `[EdDSA]`,
  so a token presenting `HS256` (the classic public-key-as-HMAC-secret attack) or `none` is rejected
  before any key is consulted. This is the highest-risk JWT footgun and is gold-tested explicitly.
- **No privilege escalation (NFR-WEB-API-SEC-02)**: the token's `sub` IS a `user_id`; the service
  computes authorization from `is_team_member(team, user_id)` / `is_workspace_admin(workspace,
  user_id)` exactly as for a human. A token therefore can NEVER do more than its bound principal can
  do in the UI. The `scope` claim / `scope_team_id` is an *additional narrowing*: if set, the
  extractor rejects (403) any request outside that team even when the bound user is a member of
  others. Scope is in the SIGNED claim, so it cannot be tampered with in transit.
- **Authorization stays in core/services, not the API tier** (NFR-WEB-API-SEC-02): the extractor
  only authenticates (who) + applies scope narrowing; the service decides authorization (may they)
  using the same store calls the UI uses. The boundary guard asserts zero
  `is_team_member`/`is_workspace_admin` call sites appear inside `foundry-api` (they belong in
  `foundry-services`).

## Issuance & revocation UX

Two surfaces, both admin-only, both reusing the existing browser auth + CSRF (these are browser
actions by a human admin, so they correctly sit on the cookie path):

- **Issue** — `POST /admin/machine-tokens` (HTML form / htmx, admin-gated like other admin routes):
  admin supplies a `name`, optional `scope_team_id`, optional expiry; the handler allocates a `jti`,
  inserts the registry row, **mints + signs the JWT** (`MachineTokenSigner::mint`), and shows the
  compact JWT **once**. Mirrors the invite-link UX (`bootstrap::create_invite`) where the secret is
  displayed once for copy-paste. (These admin handlers stay in `foundry-app` — they are HTML browser
  screens — and call a `foundry-services` issuance use-case; the signer lives behind the service so
  the HTML handler never touches key material directly.)
- **Revoke** — `POST /admin/machine-tokens/{jti}/revoke`: sets `revoked_at = now()`. Next use → 401
  via the denylist check (US-W05b scenario 2). *Note: revocation is honored only while the JWT still
  verifies (signature valid + not expired); a short-ish `exp` is the backstop if the signing key
  were ever compromised — documented as a known property of the JWT model.*
- **List** — `GET /admin/machine-tokens`: shows `name`, `issued_at`, `last_used_at`, scope, expiry,
  revoked state (never the JWT — it is not stored).

> These admin surfaces emit HTML (they are browser screens), so they live in `foundry-app`, NOT in
> `foundry-api`/`/api/v1`. The boundary guard's api≠HTML rule is therefore not in tension with them.
> A future enhancement could expose token management over JSON too, but it is out of Feature A scope.

Rotation has two senses: (a) **token rotation** = issue-new + revoke-old (no in-place primitive;
the admin governs lifecycle); (b) **signing-key rotation** = roll a new Ed25519 key into
`MACHINE_TOKEN_SIGNING_KEY` while the prior public key stays in `MACHINE_TOKEN_PUBLIC_KEYS` until the
longest outstanding `exp` passes (see §Key management). Expiry is required (`expires_at` mirrors the
JWT `exp`); the default is 90 days, admin-settable — unlike the prior opaque design there is no
"never expires," because a short-ish `exp` is the JWT model's safety backstop.

## Coexistence with sessions + CSRF (NFR-WEB-API-SEC-01, NFR-WEB-COMPAT-03/04)

The single `build_router` (in `foundry-app`) merges the cookie route group with the `/api/v1` group
that `foundry-api` contributes (via `foundry_api::routes(state)`), each with different middleware:

```text
build_router (foundry-app)
  ├── cookie group (foundry-app: /team/…, /sign-in, /admin/…, /dashboard, …)
  │     .layer(csrf_middleware)        // UNCHANGED — runs only here
  │     .layer(session_layer)          // UNCHANGED — runs only here
  └── /api/v1 group (foundry-api, .merge()-d in)
        // NO session layer, NO csrf layer; the foundry-api token_auth extractor authenticates instead
```

- The CSRF middleware already early-returns on safe methods and exempts `/bootstrap`
  (`csrf.rs:61`); the merged `/api/v1` group is simply not under the CSRF layer, so a token request
  carrying no cookie and no CSRF token is correct by construction (US-W05b scenario "needs no
  browser session or CSRF token"). **CSRF does not apply to token auth because there is no ambient
  cookie credential to abuse** — the bearer JWT must be explicitly attached, which is the standard
  reason token APIs are CSRF-exempt. This is stated and confirmed, not assumed.
- The browser path's cookie attributes, 30-day TTL, double-submit pattern, argon2id, brute-force
  delay, and non-enumerable error are **all untouched** — Feature A adds routes and a table; it edits
  no line of `session.rs`, `csrf.rs`, or `signin.rs` behavior. The existing browser acceptance
  scenarios are the proof (NFR-WEB-COMPAT-01); they must stay green as the token path is added.

## Logging hygiene (NFR-WEB-API-SEC-03)

- The minted JWT (and the Ed25519 **private** key) is wrapped in `secrecy::SecretString`, so it
  cannot be `Debug`/`Display`-logged (compile-time guard). Structured logs carry `jti` (the UUID) and
  `workspace_id` — never the JWT itself, never the signing key.
- A token-auth failure logs the *reason* (e.g. `bad_signature`, `wrong_alg`, `expired`, `revoked`)
  and `jti` only once the signature verified and a registry row was consulted; a malformed/
  bad-signature token logs the *event*, not the value. No envelope or log line ever contains the JWT
  or any key material.
- Gold test (Earned Trust): a token round-trip asserts the JWT appears in **no** log line and in
  **no** DB column (it is never persisted — only `jti` metadata is). The Ed25519 private key appears
  in no log line and no column.

## Forward-compat

`Principal` (the `Human | Machine` enum, now owned by `foundry-services`) is the single abstraction
the services consume. A future OIDC machine identity, per-endpoint OAuth scopes, multiple signing
keys with `kid` selection, or a hosted rate-limiter would extend `Principal::Machine` / the
`MachineTokenVerifier` / the extractor without touching the services or the store write paths — the
same payoff the MVP `auth.md` gets from keeping session data thin. The JWT claim set is itself a
forward-compat surface: adding an optional claim is non-breaking. All of these are explicitly out of
Feature A scope (`out-of-scope.md`).
