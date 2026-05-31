# Programmatic Foundry (Feature A) — DESIGN Wave Decisions

Owner: solution-architect (Morgan). Scope: **Feature A only** per DISCUSS D9 — first-class JSON API
(read + write) + machine-token auth + web/api/core separation + CI-enforced boundary guard
(US-W05a, US-W05b, US-W05c, US-W06). Interaction mode: **Propose**. This file is the central wave
record: decisions table, ADR-W01..W07 (MADR-style, kept here rather than as separate files —
mirroring how backend-mvp keeps ADRs close to the wave), the Reuse Analysis tally, the technology
stack with rationale, constraints, and the **Open decisions awaiting user ratification** list.

> **RATIFICATION UPDATE 2026-05-31.** The user ratified the four open DISCUSS questions. **Two
> overrode the proposed recommendations**: (1) the machine-token mechanism is now **JWT signed with
> Ed25519 (EdDSA) + a `jti` denylist** (was: opaque random + SHA-256); (2) the topology is now a
> **new `foundry-api` crate** (was: an `api` module inside `foundry-app`). The contract surface
> (`/api/v1`) and the 3-layer boundary guard stand as recommended. The affected ADRs below carry a
> **superseded-by-user-ratification (2026-05-31)** banner with the original recommendation preserved
> for the record, followed by the ratified decision. The crate split makes the boundary guard's
> dependency-direction layer Cargo-enforceable; ADR-W06 is strengthened accordingly. A new
> **ADR-W07** records where the shared `services` seam now lives (a new `foundry-services` crate),
> which the crate split forced as a sub-decision.

Output uses the LEGACY per-feature layout under `docs/feature/web-tier-extraction/design/` (NOT
`docs/product/` SSOT), per DISCUSS D6.

## Architecture summary

A new **JSON driving adapter is its own crate, `foundry-api` (`/api/v1`)** — a peer of the existing
HTML handlers in `foundry-app`, both compiled INTO the **one binary**. Both call a newly-extracted
**shared application-service crate `foundry-services`** over the **already-neutral `foundry-store`**.
The DISCUSS riskiest assumption — "core is HTML-shaped" — is **disproven by the code**:
`foundry-store` returns data rows, not HTML; the only presentation coupling is HTML `format!`
inlined in handlers. Feature A's load-bearing move is lifting the use-case orchestration into
`foundry-services::*` so two adapter crates share it — that is what makes write rule-parity
(NFR-WEB-API-CON-02) structural AND lets the boundary guard's dependency-direction layer be enforced
at the Cargo dep-graph level (`foundry-api`/`foundry-app` → `foundry-services` → `foundry-store`;
the adapter crates must NOT depend on `foundry-store` directly). Machine tokens are now **bearer JWTs
signed with Ed25519 (EdDSA)**, verified per request against an Ed25519 public key, with a
**`machine_tokens` table repurposed as an issuance registry + `jti` denylist** (revocation is a hard
requirement that stateless JWT cannot meet alone, so the per-request denylist check restores instant
revocation). Claims: `sub` (bound principal/user), `scope`, `exp`, `iat`, `jti`. The Ed25519 signing
keypair is sourced from configuration exactly as `SESSION_SECRET` is today (env → `SecretString` in
`AppState`); rotation is issue-new-key + overlap-verify (see ADR-W02 + `auth.md`). The boundary
guard (US-W06) is a custom `xtask check-arch` AST walk + `cargo-deny bans` (now with a real
crate-graph dependency-direction rule) + an injected-violation gold test proving it bites. Full
detail in `architecture.md`, `api-contract.md`, `auth.md`, `boundary-guard.md`,
`error-and-observability.md`.

## Decisions table (DDD-numbered, this wave)

> RATIFIED-OVERRIDE rows are marked **[RATIFIED 2026-05-31, OVERRODE RECOMMENDATION]**; the original
> proposed text is preserved struck-through in the relevant ADR.

| # | Decision | Rationale |
|---|---|---|
| DD1 | **[RATIFIED 2026-05-31, OVERRODE RECOMMENDATION] Topology = NEW `foundry-api` crate** (ADR-W01), a driving-adapter peer of `foundry-app`, both compiled into the one binary. | The user chose the separate crate for the stronger compile-enforced api≠HTML boundary (the api/HTML split becomes a crate-graph fact, not just an AST rule). Forces the shared seam out of `foundry-app` into a `foundry-services` crate (DD13/ADR-W07). Was: `api` module inside `foundry-app` (smaller blast radius). |
| DD2 | **Shared application-service seam — now its OWN crate `foundry-services`** that BOTH `foundry-app` HTML handlers and `foundry-api` call (ADR-W04 + ADR-W07). | The orchestration is currently inlined per-handler; with the crate split the seam can no longer live inside `foundry-app` (then `foundry-api` would have to depend on `foundry-app`, reversing the intended direction). A standalone `foundry-services` crate is the only acyclic shared home. Mechanical guarantee of rule-parity (NFR-WEB-API-CON-02) and core neutrality (NFR-WEB-BND-05). |
| DD3 | **Core is ALREADY neutral at the store layer; no core refactor needed** (ADR-W04). | Verified: `Store::list_issues_by_project`/`insert_*_with_outbox` return data, never HTML (`foundry-store/src/lib.rs`). The neutral seam exists; the work is to give the *orchestration* a shared home, not to neutralize core. |
| DD4 | **[RATIFIED 2026-05-31, OVERRODE RECOMMENDATION] Machine token = bearer JWT signed with Ed25519 (EdDSA), claims `{sub,scope,exp,iat,jti}`, verified per request against an Ed25519 public key, PLUS a `jti` denylist checked per request for instant revocation** (ADR-W02). | The user chose JWT for a standards-based, self-describing credential. Stateless JWT cannot be revoked alone, so a Postgres `jti` denylist restores the hard revocation requirement (NFR-WEB-API-SEC-02 / US-W05b). Introduces ONE new dependency (`jsonwebtoken`); "zero new deps" is no longer true and is back-propagated via `upstream-changes.md`. Was: opaque random `fdy_…` + SHA-256. |
| DD5 | **[AMENDED by DD4] `machine_tokens` table (migration `0007`) repurposed as an issuance REGISTRY + `jti` denylist — not a secret store** (ADR-W05). Forward-only, advisory-locked; new `Store` repo. | The secret is now the signed JWT itself; the table stores only `jti` (PK), bound `sub`/`user_id`, `scope`, `issued_at`, `expires_at`, `revoked_at`, audit columns — never any token secret or hash. A row's presence + `revoked_at IS NULL` + not-expired is the per-request revocation check. Follows the repo's forward-only migration discipline. |
| DD6 | **Contract surface = `/api/v1/…` path prefix, URL versioning** (ADR-W03). | Only option that makes api≠HTML structurally checkable by module path (US-W06) and keeps versioning simplest (NFR-WEB-API-CON-01). `Accept`-negotiation rejected (re-couples HTML+JSON in one handler). |
| DD7 | **Machine-token path is CSRF-exempt and session-layer-free by mounting, additive to the browser path** (ADR-W02 / auth.md). | Token auth has no ambient cookie to abuse → CSRF N/A by construction. The browser session/CSRF code is edited nowhere (NFR-WEB-API-SEC-01, NFR-WEB-COMPAT-03/04). |
| DD8 | **Boundary guard = `xtask check-arch` AST walk + `cargo-deny bans` (now a REAL crate-graph dep-direction rule) + injected-violation gold test** (ADR-W06). | api≠HTML (no `Html`/`text/html` constructed in `foundry-api`) still needs the AST layer. The crate split makes the **dependency-direction** layer Cargo-enforceable: `cargo-deny bans` now forbids `foundry-api`/`foundry-app` depending on `foundry-store` directly, and forbids `foundry-services` depending on either adapter crate. Three orthogonal layers (Principle 12c). |
| DD13 | **[NEW, ADR-W07] The shared `services` seam lives in a NEW `foundry-services` crate, NOT folded into `foundry-core`.** | Folding into `foundry-core` would force `foundry-core → foundry-store` (authz/pool), creating a dependency CYCLE (store already depends on core) and breaking core's no-I/O `#![forbid(unsafe_code)]` purity. A standalone `foundry-services` crate (depends on store + core + auth) is the only acyclic placement two adapter crates can share. This is the crux sub-decision the crate split forced. |
| DD9 | **Earned-Trust: extend the existing `store` startup probe to assert the `machine_tokens` schema; guard self-tests via injected violation** (Principle 12). | A binary booting against a pre-0007 DB must refuse to start, not fail on first token auth. The guard must prove it bites, not merely exist. |
| DD10 | **API state values are the canonical enum (`in_progress`), normalized via the SAME `normalize_state` logic, lifted into the service** (api-contract.md). | Rule-parity for inputs; one normalization source for UI + API. |
| DD11 | **No new external integration → no new contract-test surface; SMTP unchanged** (architecture.md). | The token is internal; Feature A introduces no third-party/OAuth/webhook boundary. |
| DD12 | **No OpenAPI/SDK gen, no rate-limiting, no projects/attachments/search API, no API comment-delete in Feature A** (out-of-scope.md honored). | Smallest real surface (Principle 8); deferred items have clean re-evaluation triggers already recorded in DISCUSS. |

## ADR-W01 — JSON tier topology

- **Status**: **Accepted — RATIFIED 2026-05-31. The user OVERRODE the proposed recommendation.**
- **Context**: Need a JSON tier that is a peer of the HTML handlers, in one binary, without
  destabilizing the acceptance suite (which constructs `AppState` and calls
  `foundry_app::build_router`), and that sets up Feature B's web extraction.
- ~~**Proposed (superseded)**: Add a `foundry-app::api` module mounted into the existing
  `build_router` under `/api/v1`; defer the crate split to Feature B. Rationale was smallest blast
  radius (the acceptance harness already calls `build_router`).~~ **Superseded by user ratification
  2026-05-31.**
- **Ratified decision**: Create a **new `foundry-api` crate** now, as a driving adapter exposing the
  `/api/v1` JSON surface, compiled INTO the single `foundry` binary alongside `foundry-app`. The
  user chose this for the stronger compile-enforced boundary: api≠HTML and the dependency direction
  become **crate-graph facts** (`cargo-deny`-enforceable), not solely AST conventions. The shared
  orchestration is hoisted into a new `foundry-services` crate (ADR-W07) so `foundry-api` need not
  (and must not) depend on `foundry-app`.
- **Crate graph after Feature A** (acyclic; arrows = "depends on"):
  ```text
  foundry (bin) ──> foundry-app      (HTML adapter: handlers, session, csrf, build_router)
                └─> foundry-api      (JSON adapter: /api/v1, token_auth extractor, serde)
  foundry-app   ──> foundry-services
  foundry-api   ──> foundry-services
  foundry-services ──> foundry-store, foundry-core, foundry-auth
  foundry-store ──> foundry-core
  foundry-auth  ──> foundry-core
  ```
  The adapter crates (`foundry-app`, `foundry-api`) depend on `foundry-services`, **never on
  `foundry-store` directly** (enforced — ADR-W06). `foundry-services` is the single owner of the
  pool-touching use-cases.
- **Alternatives considered**:
  - *`api` module inside `foundry-app` (the prior recommendation)*: smallest blast radius, but the
    api≠HTML boundary is only AST-enforceable (both tiers share a crate). Rejected by user in favour
    of the crate-level boundary.
  - *`foundry-api` depends on `foundry-app` for `AppState`/services*: reverses the intended
    dependency direction and re-couples the JSON tier to the HTML composition root. Rejected — this
    is exactly why the `foundry-services` crate (ADR-W07) is required.
- **Consequences**: + api≠HTML and dep-direction are crate-graph facts; Feature B's `foundry-web`
  extraction becomes a symmetric, mechanical move (the seam already lives outside the adapters).
  − Larger blast radius than the module option: a new `foundry-services` crate, the orchestration
  move into it, `AppState`/`build_router` wiring for two adapters, and ~4 acceptance `AppState`
  construction sites gain the new Ed25519-verifier field (blast radius detailed in `architecture.md`
  §Composition & harness). Mitigated: the service extraction is a pure move-and-call and the
  acceptance suite is the regression net (NFR-WEB-COMPAT-01).

## ADR-W02 — Machine-token mechanism

- **Status**: **Accepted — RATIFIED 2026-05-31. The user OVERRODE the proposed recommendation.**
- **Context**: A first-class, additive, admin-governable, revocable, scope-bounded machine
  credential (US-W05b; NFR-WEB-API-SEC-01..03) in a PG-only/no-Redis model.
- ~~**Proposed (superseded)**: Opaque 32-byte random `fdy_…` token, SHA-256 at rest, looked up per
  request on an indexed `token_hash`; zero new deps; the lookup IS the revocation check. The
  alternatives section rejected JWT/PASETO as "Option A + a dependency + footguns."~~ **Superseded
  by user ratification 2026-05-31.**
- **Ratified decision**: A **bearer JWT signed with Ed25519 (EdDSA)**. The token is the signed JWT
  itself (no server-side secret store). Per-request verification: (1) parse `Authorization: Bearer
  <jwt>`; (2) verify the **EdDSA signature** against the configured Ed25519 **public** key with the
  algorithm **pinned to `EdDSA` only** (reject any other `alg`, and reject `alg: none` — the
  alg-confusion footgun is closed by an allow-list of exactly one algorithm); (3) validate `exp`
  (and `iat`/`nbf` skew) via the library's built-in validation; (4) **check `jti` against the
  Postgres denylist/registry** — refuse if the row is absent (forged/withdrawn), `revoked_at IS NOT
  NULL` (revoked), or expired. Claims: `sub` (the bound principal/`user_id`), `scope` (team-narrowing
  filter, NULL = workspace-wide), `exp`, `iat`, `jti` (UUID). `exp` is short-ish (default 90 days,
  admin-settable) and rotation is documented (issue-new + revoke-old). Signing key management:
  the Ed25519 keypair is sourced from configuration exactly as `SESSION_SECRET` is today (env var →
  `SecretString` → `AppState`), held in a new `MachineTokenVerifier` carried in `AppState`; see
  `auth.md` §Key management and the **sub-decision flagged for ratification** below.
- **Why the `jti` denylist is mandatory**: stateless JWTs cannot be revoked by signature alone, yet
  instant revocation is a hard requirement (NFR-WEB-API-SEC-02 / US-W05b scenario 2). The denylist
  (a per-request indexed `SELECT` on `machine_tokens.jti`) restores it — verification is
  "signature valid AND `jti` is live." This is the same ~1 ms indexed-lookup cost the prior design
  had; the JWT does not buy away the DB round-trip, it adds a signature verify on top.
- **Alternatives considered (at ratification)**:
  - *Opaque + SHA-256 (the prior recommendation)*: zero new deps, simpler revocation. Not chosen by
    the user; a JWT is self-describing/standards-based and decouples verification from a per-request
    secret comparison.
  - *HS256 (symmetric) JWT*: the verifier would hold the signing secret, so any component that can
    verify can also mint — a weaker blast-radius posture. EdDSA's public/private split lets the
    verifier hold only the public key. Rejected in favour of Ed25519.
- **Consequences**: + standards-based, asymmetric (verifier holds only the public key), self-
  describing claims, instant revocation preserved via the denylist. − ONE new runtime dependency
  (`jsonwebtoken`, ADR-W05/tech-stack), new key-management + key-rotation surface, alg-confusion is
  a real footgun that MUST be closed by pinning `alg=EdDSA` (designed-in, gold-tested). The
  "zero new deps" property is lost and back-propagated to DISCUSS (`upstream-changes.md` CHG-2).

## ADR-W03 — Contract surface & versioning

- **Status**: Proposed.
- **Context**: Requests must unambiguously select JSON; the contract must be stable + versioned
  (NFR-WEB-API-CON-01/03); the boundary must be structurally checkable (US-W06).
- **Decision**: Dedicated `/api/v1/…` path prefix; URL-based versioning; JSON error envelope on all
  non-2xx; contract snapshot test as the stability net.
- **Alternatives considered**: *`Accept`-header content negotiation on existing paths* — re-couples
  HTML and JSON in one handler (defeats US-W06 and the mixed-handler separation), needs media-type
  versioning + `Vary: Accept` caching. Rejected. *Subdomain* — adds DNS/TLS/host ops surface;
  violates one-binary/no-new-infra ethos. Rejected.
- **Consequences**: + boundary checkable by module path, simplest versioning, total HTML/JSON
  separation. − paths duplicated between tiers (acceptable; they serve different consumers).

## ADR-W04 — Shared application-service seam (core neutrality)

- **Status**: Accepted (refined by the ratified crate split — see ADR-W07 for the seam's location).
- **Context**: The use-case orchestration (authz → validate → store-write → outbox) is inlined in
  each HTML handler; the API needs identical rules (NFR-WEB-API-CON-02) without duplication; core
  must be presentation-neutral (NFR-WEB-BND-05).
- **Decision**: Extract the orchestration into a shared service layer returning neutral domain
  results (never HTML/JSON), consumed by a unified `Principal` (Human | Machine). Rewire existing
  HTML handlers to call it (behavior byte-identical — render contract preserved, NFR-WEB-COMPAT-02).
  The JSON adapter calls the SAME functions. `foundry-core`/`foundry-store` are NOT refactored
  (already neutral). **Where that layer lives changed with the ratified crate split**: it is now a
  standalone `foundry-services` crate, not a `foundry-app::services` module — see **ADR-W07**.
- **Alternatives considered**: *Put services in `foundry-core`* — would pull `foundry-store`/authz
  into the pure domain crate, creating a dependency cycle (store→core already) and breaking its
  no-I/O invariant; rejected (see ADR-W07). *Let the API duplicate the orchestration* — guarantees
  drift between API and UI rules (the exact NFR-WEB-API-CON-02 failure); rejected.
- **Consequences**: + rule-parity is a compile-time fact (one function, two presentations); Feature
  B's templates reuse the same seam. − a refactor of existing handlers PLUS the move into a new
  crate (mitigated: pure move-and-call; acceptance suite is the regression net).

## ADR-W05 — Machine-token persistence (registry + jti denylist)

- **Status**: **Accepted — AMENDED 2026-05-31 to match the ratified JWT mechanism (ADR-W02).**
- **Context**: With JWT (ADR-W02) the secret is the signed token itself, so the table is NO LONGER
  a secret store. It becomes an **issuance registry + `jti` denylist** that restores instant
  revocation for an otherwise-stateless credential, and provides the admin audit/listing surface
  (NFR-WEB-API-SEC-02/03).
- **Decision**: New forward-only migration `0007_machine_tokens.sql` (advisory-locked like every
  migration). The table stores **no token secret and no hash** — only issuance/lifecycle metadata
  keyed by `jti`:
  - `jti UUID PRIMARY KEY` — the JWT's `jti` claim; the per-request denylist lookup key.
  - `workspace_id`, `user_id` (the bound `sub`/principal), `scope_team_id NULL` (narrowing),
    `name` (admin label), `created_by` (issuing admin), `issued_at`, `expires_at` (mirrors the JWT
    `exp`), `last_used_at NULL`, `revoked_at NULL`.
  - `CREATE INDEX idx_machine_tokens_workspace ON machine_tokens (workspace_id);` (the PK already
    indexes `jti`). **No `token_hash` column** — the prior design's `idx_machine_tokens_hash` is
    removed; the `jti` PK is the lookup.
  Per-request revocation check: `SELECT revoked_at, expires_at FROM machine_tokens WHERE jti=$1` →
  live iff a row exists, `revoked_at IS NULL`, and not expired. New `Store` repository methods:
  `insert_machine_token` (on issue), `find_machine_token_by_jti` (extractor), `revoke_machine_token`
  (sets `revoked_at`), `list_machine_tokens(workspace_id)`, `touch_machine_token_last_used`. The
  `store` startup probe asserts the table/columns exist (Earned Trust).
- **Alternatives considered**: *Pure stateless JWT, no table* — cannot satisfy instant revocation
  (US-W05b scenario 2). Rejected. *Reuse `bootstrap_tokens`/`reset_tokens`* — single-use, short-TTL,
  semantically wrong. Rejected. *Store the full JWT* — pointless and a needless secret-at-rest
  exposure; only `jti` is needed for the denylist. Rejected.
- **Consequences**: + clean audit + instant revocation for a stateless credential, **no secret at
  rest at all** (stronger than the prior hash-at-rest posture), probe-guarded. − a denial requires
  the JWT signature to STILL verify (revocation is only honored while the key/`exp` are valid), so a
  short-ish `exp` remains the backstop; documented in `auth.md`.

## ADR-W06 — Boundary-guard mechanism

- **Status**: Accepted (strengthened by the ratified crate split — dep-direction is now
  crate-graph-enforceable).
- **Context**: Enforce api≠HTML (now), dependency-direction (now), and web≠DB (Feature B) in CI
  (US-W06). The crate split (ADR-W01) means the **dependency-direction** invariant is no longer a
  convention — it is a crate-graph edge `cargo-deny` can forbid directly. api≠HTML still needs the
  AST layer because both `foundry-api` handlers and (future) `foundry-web` handlers could in
  principle construct an `Html(...)`; the crate boundary alone does not forbid a JSON crate from
  importing `axum::response::Html`.
- **Decision**: Keep all three orthogonal layers (Principle 12c), now mapped onto the crate split:
  1. **Structural (AST) — `xtask check-arch`**: no `axum::response::Html` / `text/html`
     content-type / `Html<…>` return is constructed anywhere in the `foundry-api` crate; no
     `is_team_member`/`is_workspace_admin` call site is added in `foundry-api` (authz lives in
     `foundry-services`). (api≠HTML and api≠ad-hoc-authz.)
  2. **Subtype/graph — `cargo-deny bans` (now a REAL rule, not just a backstop)**: forbid
     `foundry-api` and `foundry-app` from depending on `foundry-store` (they must go through
     `foundry-services`); forbid `foundry-services` from depending on `foundry-api`/`foundry-app`
     (no reversed edge); forbid `sqlx` in the (future) `foundry-web` dependency tree. The
     dependency-direction layer is the one the crate split upgraded from "AST hope" to "Cargo fact."
  3. **Behavioral (CI gold test) — injected-violation probe**: a throwaway branch introduces an
     `Html("…")` return inside a `foundry-api` handler AND a `foundry-api → foundry-store` dep edge;
     CI asserts the guard goes red on each, proving it bites (US-W06; Principle 12 self-application).
  Wired into the existing `xtask ci` + CI `lint-format` job + `deny.toml`.
- **Alternatives considered**: *`cargo-deny` only* — blind to `Html` construction inside a crate
  that is allowed to import axum. Rejected as sole mechanism (now a first-class layer for
  dep-direction). *`dylint` custom lint* — disproportionate machinery. Rejected.
- **Consequences**: + native to the repo, no new CI infra, self-tested, and the dep-direction
  invariant is now compiler/cargo-enforced (the user's stated reason for choosing the crate split).
  − the AST walk remains bespoke (small) for the intra-crate api≠HTML rule.

## ADR-W07 — Location of the shared `services` seam (forced by the crate split)

- **Status**: **Accepted — NEW 2026-05-31. Sub-decision the ratified crate split (ADR-W01) forced.**
- **Context**: With `foundry-api` as its own crate, the shared orchestration can no longer live in
  `foundry-app` (then `foundry-api → foundry-app`, reversing direction and re-coupling the JSON tier
  to the HTML composition root). The user's brief named the choice explicitly: a new
  `foundry-services` crate, OR fold orchestration into `foundry-core`.
- **Decision**: A **new `foundry-services` crate**. It depends on `foundry-store` (repositories +
  outbox + authz queries), `foundry-core` (`ProjectKey`/`IssueKey`/`render_comment_markdown`), and
  `foundry-auth` (the `Principal` machinery / JWT verifier types if shared). It exposes the
  use-cases (`board::list_board_issues`, `issues::{create,change_state}`,
  `comments::{create,edit}`), the `Principal` enum, and the `ServiceError` enum. Both `foundry-app`
  and `foundry-api` depend on `foundry-services`; neither depends on `foundry-store` directly.
- **Why NOT `foundry-core`** (the decisive rationale): `foundry-core` is pure value-objects +
  sanitization with **no I/O** and `#![forbid(unsafe_code)]`; it is depended-upon by `foundry-store`
  and `foundry-auth`. Folding orchestration into it would force `foundry-core → foundry-store` (to
  reach the pool and authz queries), which is a **dependency cycle** (`foundry-store → foundry-core`
  already exists) AND destroys core's no-I/O purity. Cyclic + purity-breaking = disqualifying.
  `foundry-store` already provides the neutral repositories; the missing thing is an
  orchestration home that may depend on the store — which is precisely what a layer *above* store and
  *below* the adapters is. That layer is `foundry-services`.
- **Alternatives considered**: *Fold into `foundry-core`* — dependency cycle + breaks no-I/O purity;
  rejected. *Fold into `foundry-store`* — would put authz/validation/use-case policy into the
  persistence adapter, conflating "how we talk to Postgres" with "what the use-case decides";
  rejected (keeps store a thin neutral repository). *Keep in `foundry-app`* — reverses the dep
  direction for `foundry-api`; rejected (the reason the crate split needed this ADR at all).
- **Consequences**: + the only acyclic placement two adapter crates can share; makes dep-direction
  a `cargo-deny` fact (ADR-W06 layer 2); Feature B's `foundry-web` reuses the same seam with no
  further move. − one more crate in the workspace (`Cargo.toml` members) and the orchestration move
  is the largest single mechanical change in Feature A (pure move-and-call; suite is the net).

## Reuse Analysis tally (full table in `architecture.md`)

- **EXTEND = 9** (board read, issue/state/comment writes + outbox, sanitization, authz checks,
  router composition, session/CSRF scope-around, startup probe, boundary CI infra). *(Down from 11:
  the two `foundry-auth` rows that previously EXTENDED the `BootstrapToken` random+SHA-256 pattern
  and the HMAC/constant-time path no longer apply — the JWT credential does not reuse that pattern;
  the HMAC `sign`/`verify` path is untouched and simply not a Feature-A component anymore. They are
  removed from the table rather than reclassified.)*
- **CREATE NEW = 6**, each with no existing alternative: (1) the **`foundry-api` crate** (JSON
  adapter — was a module, now a crate); (2) the **`foundry-services` crate** (the shared seam by
  *extraction* of inlined logic, ADR-W07 — was a module, now a crate); (3) the JWT machine-token
  mint/verify primitive (`foundry_auth::MachineToken` → now an Ed25519 sign/verify type, ADR-W02);
  (4) the `machine_tokens` registry/`jti`-denylist table + repo (ADR-W05); (5) the `jsonwebtoken`
  **runtime dependency** (net-new; tech stack below); (6) the Ed25519 key-management config seam
  (`MachineTokenVerifier` in `AppState`, sourced like `SESSION_SECRET`).
- The entire write/read/authz/sanitization/outbox core is **still 100% reused** — the mechanical
  basis for rule-parity is unchanged by the override; only the credential mechanism and the crate
  topology changed.

## Technology stack (versions pinned; OSS-first; from `Cargo.lock`/workspace where possible)

| Concern | Choice | Version (workspace) | License | New dep? |
|---|---|---|---|---|
| HTTP / routing | `axum` | 0.8 | MIT | no (existing) |
| JSON (de)serialization | `serde` + `serde_json` | 1 | MIT/Apache-2.0 | no (existing) |
| **JWT encode/verify (EdDSA)** | **`jsonwebtoken`** | **9 (workspace pin `"9"`; ≥9.3 resolved)** | **MIT** | **YES — NET NEW (the only one)** |
| JWT `jti` / claims IDs | `uuid` | 1.23 | MIT/Apache-2.0 | no (existing) |
| Base64 (token + key encoding) | `base64` | 0.22 | MIT/Apache-2.0 | no (existing) |
| Secret handling (Ed25519 private key, JWT) | `secrecy` | 0.10 | MIT/Apache-2.0 | no (existing) |
| Persistence | `sqlx` (Postgres) | 0.8 | MIT/Apache-2.0 | no (existing) |
| Sanitization (reused) | `ammonia` + `pulldown-cmark` | 4 / 0.12 | MIT / MIT | no (existing) |
| Boundary guard | `xtask` (custom) + `cargo-deny` | — / CI | AGPL-3.0 (repo) / — | no new runtime dep |
| Observability | `tracing` + `metrics` + `metrics-exporter-prometheus` | 0.1 / 0.23 / 0.15 | MIT/Apache-2.0 | no (existing) |

> Note: `rand`/`sha2`/`subtle`/HMAC were the primitives the *prior* opaque-token design reused; the
> JWT design does not use them for the machine token (they remain in `foundry-auth` for argon2id /
> bootstrap / invite, untouched). They are dropped from this table as they are no longer Feature-A
> components.

**Net new dependencies introduced by Feature A: ONE — `jsonwebtoken = "9"` (MIT).** This is the
direct consequence of the ratified JWT/Ed25519 decision (ADR-W02), which **overrode** the prior
zero-new-deps recommendation. Pinning + transitive surface, grounded in `Cargo.lock`:

- `jsonwebtoken = "9"` (latest 9.x resolves to ≥9.3) supports `Algorithm::EdDSA`. Its default
  feature set uses **`ring`** for crypto — and **`ring` 0.17.14 is ALREADY in `Cargo.lock`**
  (pulled transitively by `rustls`/`sqlx tls-rustls`), so EdDSA verification rides an already-vetted
  crypto backend rather than adding `aws-lc-rs`/`ed25519-dalek` (neither is currently in the tree).
  Keep `jsonwebtoken` on **default features (ring)**; do NOT enable its `aws_lc_rs` feature, to avoid
  adding a second crypto stack.
- Genuinely net-new transitive crates are therefore small: `jsonwebtoken` itself, plus its
  JWT-specific helpers (`simple_asn1`, `pem`) — `base64`, `serde`, `serde_json`, `ring`, `time`,
  `pkcs8`, `spki` are already present. `cargo deny check` must be re-run; `jsonwebtoken` (MIT) and
  its deps are in the already-allowed license set, so no `deny.toml` license change is anticipated
  (verify on the actual lock update).
- Add to `crates/foundry-auth/Cargo.toml` (the natural home for the credential primitive) and to the
  workspace `[workspace.dependencies]` as `jsonwebtoken = "9"`.

## Constraints honored (NFR traceability)

- One binary, one Postgres, no Redis, no new runtime *service*, no network hop between tiers
  (NFR-WEB-BND-04, NFR-WEB-INFRA-01, NFR-WEB-PERF-02) — `foundry-api` is a new CRATE compiled INTO
  the single binary, NOT a separate process. NFR-WEB-INFRA-01's "no new runtime service" holds; its
  "no new dependency" clause is **partially broken** by the ratified JWT choice (one new compile-time
  crate, `jsonwebtoken`) — reconciled in `upstream-changes.md` CHG-2.
- API and HTML call the SAME `foundry-services` → SAME core writes/authz/validation/sanitization/
  outbox (NFR-WEB-API-CON-02, NFR-WEB-BND-03/05).
- Machine-token auth is additive; browser session/CSRF/password path unchanged
  (NFR-WEB-API-SEC-01, NFR-WEB-COMPAT-03/04). Token requests are CSRF-exempt by construction.
- JSON handlers emit JSON only; errors are JSON (NFR-WEB-API-CON-03, NFR-WEB-BND-02) — boundary-guard
  enforced (api≠store dep-direction now crate-level).
- Tokens revocable (via `jti` denylist), scope-bounded, **no secret at rest** (the signed JWT is the
  secret; only `jti` metadata persists), never logged (NFR-WEB-API-SEC-02/03).
- Stable versioned contract with a snapshot test (NFR-WEB-API-CON-01).
- Existing acceptance suite stays green; boundary guard is CI-enforceable (NFR-WEB-COMPAT-01,
  US-W06).
- Default architecture preserved: modular monolith with dependency inversion (ports-and-adapters);
  the ratified topology is the stronger crate-split form (ADR-W01) — still one binary.

## Priority validation (reviewer Dimension 5)

- **Q1 largest bottleneck?** The confirmed primary driver (D7) is "external agents can drive Foundry
  programmatically." The design leads with exactly that surface (the JSON read+write API + token
  auth). YES.
- **Q2 simpler alternatives considered?** Yes — ADR-W01 documents the simpler module option (which
  the user weighed and overrode in favour of the crate split); ADR-W02 documents the simpler
  opaque-token option (overridden in favour of JWT); ADR-W04/W07 document 2 wrong service homes.
  *Note: the prior bias-check flagged JWT as "resume-driven"; the user ratified JWT deliberately for
  a standards-based asymmetric credential, so the DESIGN now records JWT as a ratified trade-off, not
  a bias. The honest cost (one new dep + alg-confusion footgun + key management) is stated, not
  hidden.*
- **Q3 constraint prioritization?** The riskiest assumption (core neutrality) is addressed FIRST and
  found already-satisfied at the store layer; the real cost (service extraction) is sized as a
  pure refactor with the acceptance suite as the net. Not inverted.
- **Q4 data-justified?** The "core is neutral" claim is grounded in specific file:line evidence
  (`foundry-store/src/lib.rs` return types), not assumption.

## Ratification status (updated 2026-05-31)

The four DISCUSS open questions are **ratified**. Two overrode the recommendation (marked OVERRIDE).

1. **Machine-token mechanism (Open Q1)** — **RATIFIED: JWT signed with Ed25519 (EdDSA) + `jti`
   denylist** (ADR-W02). *OVERRODE the recommendation (opaque + SHA-256). Revocation preserved via
   the `jti` denylist; one new dep (`jsonwebtoken`).*
2. **JSON contract surface (Open Q2)** — **RATIFIED: `/api/v1/…` path prefix, URL versioning**
   (ADR-W03). *Matches the recommendation — no change.*
3. **Crate topology (Open Q3)** — **RATIFIED: a NEW `foundry-api` crate now** (ADR-W01). *OVERRODE
   the recommendation (`api` module inside `foundry-app`). Forces the new `foundry-services` crate
   (ADR-W07).*
4. **Core neutrality (Open Q4)** — **RATIFIED: no core refactor; extract the shared seam** (ADR-W04)
   — but the seam now lives in a new `foundry-services` crate, not a `foundry-app::services` module
   (ADR-W07, consequence of #3). *Substance unchanged; location changed by the topology override.*
5. **Boundary-guard mechanism** — **RATIFIED: the 3-layer guard** (ADR-W06), with the dependency-
   direction layer strengthened to a real `cargo-deny` crate-graph rule (exploits #3). *Matches the
   recommendation.*
6. **Token persistence** — **RATIFIED (amended for JWT): `0007_machine_tokens.sql` as a registry +
   `jti` denylist (no secret/hash at rest), probe-guarded** (ADR-W05).
7. **Optional API metrics** — RECOMMEND bounded `machine_token_auth_failures_total{reason}` +
   `machine_tokens_active`; `reason` set extended for JWT (`+ invalid_signature`,
   `+ wrong_alg`). *Still non-blocking; user may defer.*

### NEW sub-decision flagged for user ratification (Ed25519 key storage & rotation)

The JWT override introduces signing-key management, which was not previously a decision surface.
DESIGN proposes — and asks the user to ratify — the following, grounded in Foundry's existing config
model (`main.rs` reads everything from env via `std::env::var` + `dotenvy`; `SESSION_SECRET` is the
direct precedent: required, ≥32 bytes, wrapped in `SecretString`, placed in `AppState`):

- **Storage**: the Ed25519 keypair lives in **configuration (env vars), exactly like
  `SESSION_SECRET`** — NOT in Postgres, NOT in a committed file. `MACHINE_TOKEN_SIGNING_KEY`
  (Ed25519 private key, base64/PKCS8 PEM, `SecretString`) is required only on the binary that ISSUES
  tokens; `MACHINE_TOKEN_PUBLIC_KEY` (the public key) is required on every binary that VERIFIES.
  Single-binary deployments set both. A `MachineTokenVerifier` (holds the parsed public key + the
  `EdDSA`-pinned `Validation`) is built at boot and placed in `AppState` alongside `session_secret`.
- **Rotation**: issue-new-key + overlap. The verifier accepts a SMALL SET of public keys
  (`MACHINE_TOKEN_PUBLIC_KEYS`, comma-separated, newest first) so a new signing key can be rolled in
  while tokens signed by the prior key still verify until they expire; the old public key is dropped
  after the longest outstanding `exp`. No `kid` header is required for Feature A's single-issuer case
  but the verifier tries each configured public key (cheap; ≤2 in practice).
- **Why config, not Postgres**: putting the private key in the DB co-locates the signing secret with
  the data it protects and complicates the probe/boot story; env-var secrets match the existing
  posture and keep the key out of the data plane. Why not a file: env is how every other secret
  (DB creds, `SESSION_SECRET`) already arrives; a file adds a second secret-delivery mechanism.

**Ratify**: (a) keypair in env (like `SESSION_SECRET`) vs Postgres vs file; (b) rotation via
overlapping public-key set vs a single key with hard cutover. DESIGN recommends env + overlapping
set. This is the one genuinely-new sub-decision the JWT override created.

> **RATIFIED 2026-05-31 (user-confirmed).** (a) **Env / config** storage — `MACHINE_TOKEN_SIGNING_KEY`
> (private, issuer) + `MACHINE_TOKEN_PUBLIC_KEYS` (verifiers), loaded at boot like `SESSION_SECRET`,
> with the sign+verify startup probe refusing boot on bad key material. (b) **Overlapping public-key
> set** rotation — verifiers accept ≤2 keys; issue with the new key; drop the old public key after the
> longest outstanding `exp`. Zero-downtime, no mass token invalidation. DESIGN's recommendation was
> accepted as-is; no further key-management decisions are open.

## DISCUSS assumptions challenged

One. The DISCUSS risk register and slice-1 hypothesis frame the headline risk as "core might be
secretly HTML-shaped." Reading the code shows the store layer is **already neutral**; the actual
coupling is inlined orchestration in handlers. This refines (does not contradict) the DISCUSS
framing and changes the slice-1 work from "prove core can feed JSON" to "extract the shared service
seam and prove both adapters call it." Recorded in `upstream-changes.md`. No story or scope change
results; the walking-skeleton value (a real JSON read from the same core path) is unchanged.
