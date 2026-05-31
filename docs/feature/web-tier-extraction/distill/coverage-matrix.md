# Coverage Matrix — Feature A "Programmatic Foundry" (web-tier-extraction)

Scope: Feature A only per DISCUSS D9 — US-W05a (JSON read), US-W05b (machine-token
auth), US-W05c (JSON writes), US-W06 (boundary guard). Slice 1 = US-W05a (walking
skeleton). Slice 2 = US-W05b + US-W05c + US-W06 (folded in).

Proves: every in-scope AC and every NFR linked to a Feature-A story is covered by at
least one scenario; every DESIGN driving adapter (`/api/v1` routes + the boundary
guard) maps to ≥1 scenario; every driven adapter has a `@real-io` scenario. Tags are
the handles DELIVER uses to run subsets.

## Scenario inventory

| File | Scenarios | Happy/regression | Error/edge | Tags |
|---|---|---|---|---|
| `us-w05a-api-read-issues.feature`     | 4 | 3 | 1 | `@feature-a @us-w05a @walking_skeleton @real-io @driving_adapter @error` |
| `us-w05b-machine-token-auth.feature`  | 10 | 3 | 7 | `@feature-a @us-w05b @slice2-entry @real-io @driving_adapter @error @nfr-web-api-sec-01 @nfr-web-api-sec-02` |
| `us-w05c-api-write-issues.feature`    | 6 | 4 | 2 | `@feature-a @us-w05c @slice2-entry @real-io @driving_adapter @error @nfr-web-api-con-02` |
| `us-w06-boundary-guard.feature`       | 4 | 1 | 3 | `@feature-a @us-w06 @infrastructure @boundary-guard @real-io @error @nfr-web-api-sec-02` |
| **Total** | **24** | **11** | **13** | |

**Error/edge ratio: 13/24 = 54%** (target ≥40% — met). Auth (US-W05b) and the guard
(US-W06) are refusal-heavy by nature, which is correct: a credential's truth table and
a guard's job are mostly "say no."

**Walking skeleton: exactly ONE for the feature** — `us-w05a-api-read-issues.feature`
→ *"An integrator reads the board's issues as data"* (`@walking_skeleton @real-io
@driving_adapter`). It is the single demo-able end-to-end skeleton (story-map.md
§Walking Skeleton: Slice 1). The first scenario of US-W05b and US-W05c carries
`@slice2-entry` instead — they are the SLICE-2 entry proofs (the first thing DELIVER
makes pass in Slice 2), not feature walking skeletons. (Resolved during the DISTILL
self-review: there is now exactly one `@walking_skeleton` per feature, satisfying
critique Dimension 5.)

## Story → AC → Scenario trace

### US-W05a (5 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| ≥1 read endpoint serves the board's issues as JSON via foundry-api | `An integrator reads the board's issues as data` |
| The JSON tier reuses the same core/store call the web tier uses | `The data answer and the browser board come from the same core path` |
| No foundry-api handler emits HTML | `...as data` (asserts no markup) + US-W06 `A page constructed in the data-API tier fails the check` |
| The endpoint enforces authorization (membership) equivalent to the web tier | `A request with no valid credential is refused` |
| Empty result returns `[]` with 200, not error or HTML | `An empty project answers with an empty list` |

### US-W05b (5 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| API accepts a machine token as a first-class credential, distinct from the browser path | `A machine reads with its granted credential` |
| Admin can issue + revoke; a revoked token is refused | `A revoked credential is refused on its next use` (+ the granting Given on every happy scenario) |
| Token-authed request has the SAME authorization as the bound human (no escalation) | `A credential cannot reach beyond the team it was scoped to` |
| A machine-token request needs neither a session cookie nor a CSRF token | `A machine credential needs no browser session and no anti-forgery token` |
| The browser session/CSRF model is unchanged (additive surface) | `The browser sign-in path is unchanged by the machine credential surface` |
| (Fail-closed catalogue — auth.md / error-and-observability.md) | `no credential` / `malformed` / `forged` / `expired` / `disallowed algorithm` (5 refusal scenarios, all 401 non-enumerable) |

### US-W05c (6 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| JSON write endpoints: create issue, update state, create comment, edit comment | `A machine files an issue` + `...moves an issue to a new state` + `A comment posted through the API...` + (edit covered by the 403 parity scenario) |
| Writes reuse the SAME core write functions (incl. outbox) | `An issue filed through the API appears to a member watching the board` (asserts same core path) |
| API writes enforce the SAME authorization as the UI | `A write beyond the credential's authorization is refused` |
| API writes enforce the SAME validation + (comments) core sanitization | `An issue with an empty title is rejected by the same rule the browser enforces` + `A comment posted through the API is sanitized exactly as a browser comment` |
| Successful writes return the resource as JSON; errors are JSON, never HTML | `A machine files an issue` (no markup) + `...rejected... returned as data with no markup` |
| An API-created change reaches realtime/SSE consumers via the same outbox | `An issue filed through the API appears to a member watching the board` |

### US-W06 (3 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| CI fails when an adapter crate depends on the DB pool directly | `An adapter reaching the database directly fails the check` (cargo-deny layer) |
| CI fails when a foundry-api handler returns an HTML body | `A page constructed in the data-API tier fails the check` (AST layer) |
| Guard runs in the existing CI lane; a clean boundary passes without manual steps | `A clean tree passes the boundary check` |
| (DESIGN-added) verifier must pin alg=EdDSA; non-EdDSA fails the guard | `A credential verifier that would accept a disallowed algorithm fails the check` |

## NFR → Scenario trace (Feature A NFRs, per nfrs.md NFR-MATRIX)

| NFR | Linked stories | Scenario(s) |
|---|---|---|
| NFR-WEB-API-CON-01 (stable versioned contract) | W05a, W05c | route lives under `/api/v1`; contract-snapshot test is a DELIVER unit/integration concern (flagged below) |
| NFR-WEB-API-CON-02 (write rule-parity) | W05c | `...sanitized exactly as a browser comment` + `rejected by the same rule the browser enforces` + `appears to a member watching the board` |
| NFR-WEB-API-CON-03 (no format leaks; errors are JSON) | W05a, W05c, W06 | `...contains no markup` (read) + `rejection returned as data with no markup` (write) + US-W06 page-violation |
| NFR-WEB-API-SEC-01 (machine token additive; browser unchanged) | W05b | `The browser sign-in path is unchanged by the machine credential surface` |
| NFR-WEB-API-SEC-02 (revocable + scope-bounded; no escalation; alg pin) | W05b, W05c, W06 | `revoked... refused` + `cannot reach beyond the team` + `disallowed algorithm refused` + US-W06 alg-pin guard |
| NFR-WEB-API-SEC-03 (secrets handled like credentials; never logged/persisted) | W05b | DELIVER assertion: JWT in no log line + no DB column (the gold test, auth.md §Logging hygiene). Not user-observable in Gherkin — flagged below as a DELIVER integration test. |
| NFR-WEB-BND-02 (api emits no HTML) | W05a, W05c, W06 | every "no markup" assertion + US-W06 page-violation |
| NFR-WEB-BND-04 (one binary, in-process, no hop) | all | structural — foundry-api is a crate compiled into one binary; no inter-tier socket. Verified by the @docker-compose topology check (deferred to DEVOPS/DELIVER; see gaps). |
| NFR-WEB-BND-05 (core presentation-neutral) | W05a | `The data answer and the browser board come from the same core path` |
| NFR-WEB-COMPAT-01 (existing acceptance suite stays green) | all | the whole pre-existing suite is the regression net; this DISTILL adds non-breaking scaffolds only (verified: workspace compiles, no existing scenario edited) |
| NFR-WEB-COMPAT-03/04 (CSRF + session contract unchanged) | W05b | `The browser sign-in path is unchanged...` (+ existing US-06 scenarios stay green) |

## Driving adapter coverage (Mandate 1 + Driving Adapter Verification)

DESIGN entry points (api-contract.md route surface + boundary-guard.md):

| Driving adapter | Protocol | Scenario(s) invoking it |
|---|---|---|
| `GET /api/v1/teams/{team}/projects/{project}/issues` | HTTP GET, bearer JWT | US-W05a all 4 + US-W05b all (the read is the auth probe surface) |
| `POST .../issues` | HTTP POST JSON | US-W05c `files an issue` + `empty title` |
| `PATCH .../issues/{number}` | HTTP PATCH JSON | US-W05c `moves an issue to a new state` |
| `POST .../issues/{number}/comments` | HTTP POST JSON | US-W05c `comment ... sanitized` |
| `PATCH .../issues/{number}/comments/{comment_id}` | HTTP PATCH JSON | US-W05c `write beyond the credential's authorization` |
| Boundary guard (`cargo xtask check-arch` + `cargo-deny`) | subprocess against the source tree | US-W06 all 4 |

Every DESIGN driving adapter in Feature-A scope is exercised via its real protocol
(HTTP for the API, subprocess for the guard) — not substituted by a service-level call.
Mandate 1 satisfied.

## Driven adapter coverage (Mandate 6)

Per the project Infrastructure Policy (`docs/architecture/atdd-infrastructure-policy.md`)
and Architecture of Reference.

Driven internal (real, via testcontainers Postgres + per-scenario schema):

| Adapter | `@real-io` scenario | Covered by |
|---|---|---|
| `PgPool` issues / projects / teams / memberships / outbox | YES | every US-W05a/b/c `@real-io` scenario |
| `machine_tokens` registry / `jti` denylist (NEW, migration 0007) | YES — issue/revoke/forged/expired paths | US-W05b revoke + forged + expired scenarios (once DELIVER lands the repo) |
| tower-sessions Postgres store (browser path, unchanged) | YES | US-W05b `browser sign-in path is unchanged` |
| real markdown sanitizer in `foundry-core` | YES | US-W05c `comment ... sanitized exactly as a browser comment` |
| SSE/outbox broadcast | YES | US-W05c `appears to a member watching the board` |

Driven external / non-deterministic (fakes per policy):

| Adapter | Coverage strategy |
|---|---|
| Ed25519 signing keypair (NEW) | a fixed test keypair set in `AppState` at harness build (mirrors the fixed `session_secret` test value); the EdDSA verify is real crypto, not faked |
| clock (token `exp`) | the expired-credential scenario uses the existing `MockClock` to age a token past `exp` (DELIVER wires) |

The boundary guard (US-W06) is itself a `@real-io` subprocess against the real source
tree — it has no driven Postgres adapter; its "real I/O" is the filesystem + cargo.

## Coverage gaps (flagged for DELIVER)

1. **NFR-WEB-API-CON-01 contract-snapshot test**: the stable-versioned-contract guarantee
   is a recorded-response structural snapshot (api-contract.md §Versioning). Best as a
   `foundry-api` integration/unit test, NOT an acceptance scenario. **Resolution: DELIVER
   adds a snapshot test; not an acceptance gap.**
2. **NFR-WEB-API-SEC-03 secret hygiene (JWT never logged/persisted)**: not user-observable
   in the Gherkin sense. **Resolution: DELIVER gold test (auth.md §Logging hygiene) asserts
   the JWT appears in no log line and no DB column.**
3. **NFR-WEB-BND-04 one-binary/no-hop topology**: best proven by a `@docker-compose`
   topology assertion (one foundry container + Postgres), mirroring backend-mvp US-01.
   **Resolution: add a `@docker-compose` scenario in DEVOPS/DELIVER or fold into the
   existing US-01 compose check; out of this DISTILL's in-process scope.**
4. **US-W05c comment-edit happy path**: the edit endpoint is covered by the 403 non-author
   refusal but not a happy author-edit. **Resolution: cheap to add in Slice-2 DELIVER (one
   scenario); the create/state/comment-create happy paths already exercise the write seam.**
5. **Machine-token issuance admin UX** (`POST /admin/machine-tokens`, HTML, in foundry-app):
   out of this acceptance set's API scope — it is a browser admin screen. **Resolution: the
   granting is exercised structurally via the `admin has granted a credential` Given; the
   admin HTML screen is a Feature-A browser surface tested in foundry-app's own tests.**

## Tag schema (consumed by DELIVER + CI)

| Tag | Purpose |
|---|---|
| `@feature-a` | All four files carry it — `--tags @feature-a` runs the whole feature |
| `@us-w05a` `@us-w05b` `@us-w05c` `@us-w06` | Story scope |
| `@walking_skeleton` | The single US-W05a demo skeleton (exactly one per feature) |
| `@slice2-entry` | The first scenario DELIVER makes pass in Slice 2 (US-W05b read + US-W05c create) |
| `@driving_adapter` | Scenarios entering via the real `/api/v1` HTTP protocol |
| `@real-io` | Real Postgres / real subprocess / real crypto |
| `@error` | Refusal / rejection / violation scenarios (13 of 24) |
| `@boundary-guard` | US-W06 subprocess scenarios — shard onto the lint lane |
| `@infrastructure` | US-W06 (no standalone user value; rides Slice 2) |
| `@nfr-web-api-sec-01/02`, `@nfr-web-api-con-02` | NFR-driven scenarios |

> No `@property` scenarios in Feature A: the API surface is example-shaped (config/CRUD),
> not domain-rich-input-shaped at the acceptance layer (Mandate 9 — layer 3+ is
> example-only anyway). Property-based coverage belongs to DELIVER's `foundry-services`
> unit tests (layer 1-2): `normalize_state` round-trips, title-validation boundaries, the
> EdDSA verify truth table. Flagged for DELIVER, not authored here.
