# Web-Tier Extraction — Non-Functional Requirements

> DRIVER-CORRECTED (2026-05-30). The JSON API is now first-class (read + write, machine-token
> auth), so this file adds an **NFR-WEB-API** section (versioned contract, content negotiation,
> machine-token security, write rule-parity) ahead of the boundary/web NFRs. Cross-cutting
> requirements for the web/api separation. Each NFR is testable, measurable, and traceable to a
> job. These extend (do not replace) the backend-mvp NFRs in
> `docs/feature/foundry-backend-mvp/discuss/nfrs.md`; where this feature must preserve an
> existing NFR unchanged, that is stated explicitly.

> **Source of truth**: `stories.md` for functional behavior; this file for NFRs;
> `out-of-scope.md` for explicitly deferred items.

---

## NFR-WEB-API — JSON API as a first-class surface (PRIMARY — jtbd-web-4)

### NFR-WEB-API-CON-01: Stable, versioned JSON contract
- **Requirement**: The JSON API exposes a versioned contract so integrations and agents do not
  break when the UI or internal models evolve. The response shape for a given versioned endpoint
  is stable; breaking changes require a new version, not a silent change. (The versioning
  MECHANISM — URL prefix vs header vs media type — is DESIGN; the stability guarantee is fixed.)
- **Test**: A recorded JSON response for a versioned endpoint remains structurally valid across
  a UI/markup change (contract snapshot test); a breaking field change fails the snapshot.
- **Linked stories**: US-W05a, US-W05c.
- **Job link**: jtbd-web-4 (an integrator can build on the contract without fear of UI drift).

### NFR-WEB-API-CON-02: API writes have rule-parity with the UI
- **Requirement**: A create/update performed through the JSON API enforces the SAME
  authorization (membership/admin/authorship), the SAME validation (e.g. "title required"), and
  the SAME markdown sanitization (in core) as the equivalent browser action, and travels the
  SAME outbox so realtime consumers see API and UI writes identically. The API is not a
  privileged or divergent back door.
- **Test**: Paired acceptance scenarios assert that an API write and a UI write produce the same
  acceptance/rejection outcome and the same sanitized stored content; an API-created issue
  appears on a watching UI board.
- **Linked stories**: US-W05c.
- **Job link**: jtbd-web-4 (writes are trustworthy and consistent with the product).

### NFR-WEB-API-CON-03: Content negotiation is unambiguous; no format leaks
- **Requirement**: A request unambiguously selects JSON or HTML (via Accept header and/or `/api`
  path prefix — DESIGN picks). JSON handlers MUST NOT emit HTML (including HTML error pages) and
  web handlers MUST NOT emit JSON. Errors on the JSON surface are JSON.
- **Test**: Every JSON endpoint response (success and error) parses as JSON and contains no HTML
  tags; the boundary guard (US-W06) enforces api≠HTML structurally.
- **Linked stories**: US-W05a, US-W05c, US-W06.
- **Job link**: jtbd-web-4 + jtbd-web-2.

### NFR-WEB-API-SEC-01: Machine-token authentication is first-class and additive
- **Requirement**: The API accepts a machine token as a first-class credential, distinct from
  the browser cookie/session+CSRF path, and the existing browser auth model is UNCHANGED by its
  addition. A machine-token request requires neither a session cookie nor a CSRF token.
  (Token format/storage/issuance/rotation/revocation/scoping = DESIGN.)
- **Test**: A request with only a valid machine token succeeds against an API endpoint; the
  browser-path acceptance scenarios (session + CSRF) remain green unchanged.
- **Linked stories**: US-W05b.
- **Job link**: jtbd-web-4.

### NFR-WEB-API-SEC-02: Machine tokens are revocable and scope-bounded
- **Requirement**: A workspace admin can issue and revoke machine tokens; a revoked token is
  refused on its next use; a token cannot exceed the authorization of the principal/scope it is
  bound to (no privilege escalation). Authorization decisions stay in core, not the API tier.
- **Test**: Revoked-token request → unauthorized; a token scoped to one team is forbidden on
  another team's resource; zero authorization logic in the API tier (decisions come from core).
- **Linked stories**: US-W05b, US-W05c.
- **Job link**: jtbd-web-4 (governable machine access).

### NFR-WEB-API-SEC-03: Machine-token secrets are handled like credentials
- **Requirement**: Machine tokens are treated as secrets: not stored in plaintext at rest, not
  logged, and transmitted only over the existing transport security posture. (Exact hashing/
  storage = DESIGN; the handling constraint is fixed here so DESIGN cannot regress it.)
- **Test**: No token plaintext appears in logs or the database in a token round-trip; reuses the
  backend-mvp secret-handling posture.
- **Linked stories**: US-W05b.
- **Job link**: jtbd-web-4 (the new auth surface is as safe as the model it complements).

---

## NFR-WEB-BND — Boundary Honesty (the core architectural NFRs)

### NFR-WEB-BND-01: Web tier has no direct database access
- **Requirement**: `foundry-web` MUST NOT depend on the database connection pool or run SQL.
  All data reaches the renderer through `foundry-core`/`foundry-store` calls. The web tier
  produces HTML; it does not query Postgres.
- **Test**: Crate-graph assertion — `foundry-web` has no dependency that exposes the pool;
  zero `sqlx::query*` call sites under the web tier. Enforced by US-W06 in CI.
- **Linked stories**: US-W01, US-W06.
- **Job link**: jtbd-web-2.

### NFR-WEB-BND-02: API tier emits no HTML
- **Requirement**: `foundry-api` MUST NOT return HTML bodies. Its responses are JSON (or
  other machine formats), never rendered markup.
- **Test**: Response-type/content-type assertion; injected HTML-return PR fails the guard.
- **Linked stories**: US-W05, US-W06.
- **Job link**: jtbd-web-2.

### NFR-WEB-BND-03: Sanitization and authorization stay in core, not the template
- **Requirement**: Markdown sanitization (`foundry_core::render_comment_markdown`, ammonia)
  and authorization decisions (membership, admin, authorship) remain in core/store. The web
  tier renders the *result* (sanitized HTML, boolean affordance flags); it performs neither.
- **Test**: Zero ammonia/sanitization and zero `is_team_member`/`is_workspace_admin` call
  sites under the web tier (templates receive pre-sanitized HTML and pre-decided flags).
- **Linked stories**: US-W03.
- **Job link**: jtbd-web-2 (security-critical logic does not migrate into presentation).

### NFR-WEB-BND-04: One binary, in-process, no network hop
- **Requirement**: `foundry-web` and `foundry-api` are PEER modules inside the single `foundry`
  binary. Calls between web/api and core/store are in-process function calls — there is NO
  HTTP/RPC hop between tiers, NO second service, NO Redis, NO Node runtime service. Adding the
  JSON API tier does NOT introduce a second process.
- **Test**: `docker compose up` runs exactly one foundry container (plus Postgres) as today;
  no inter-tier socket appears in the deployment.
- **Job link**: jtbd-web-3 + jtbd-web-4 + the README "one binary, one Postgres" brand promise.

### NFR-WEB-BND-05: Core is presentation-neutral (feeds web AND api equally)
- **Requirement**: `foundry-core`/`foundry-store` MUST be free of HTML- and JSON-specific
  assumptions — the same core call serves the HTML board and the JSON board endpoint. Neither
  presentation format is privileged in core; serialization happens in the respective tier.
- **Test**: The web board render and the JSON board endpoint obtain their data through the SAME
  core call (US-W05a scenario); zero `format!`-HTML and zero serde-JSON in core for that path.
- **Linked stories**: US-W05a, US-W01.
- **Job link**: jtbd-web-2 (the enabler that makes jtbd-web-4 first-class).

---

## NFR-WEB-PERF — Performance (no regression)

### NFR-WEB-PERF-01: Template rendering stays within the existing render budget
- **Requirement**: P95 server-render latency for the board, issue page, and sign-in,
  rendered via templates, is ≤200 ms at the application boundary on the backend-mvp reference
  hardware (2 vCPU, 4 GB, Postgres on host, ≤1 ms DB RTT) — i.e. the extraction adds no
  measurable regression vs the current `format!` path.
- **Test**: `criterion` bench on the template render path vs a baseline; synthetic HTTP load
  (50 RPS, 1,000 issues seeded). Reuses NFR-PERF-01's harness from backend-mvp.
- **Linked stories**: US-W01, US-W02, US-W03, US-W04.
- **Job link**: jtbd-outcome-4 (Linear-feel speed must not degrade).

### NFR-WEB-PERF-02: No inter-tier latency added
- **Requirement**: Because tiers are in-process (NFR-WEB-BND-04), the web/api separation adds
  0 ms of network latency between presentation and core. The separation is a compile-time
  organization, not a runtime cost.
- **Test**: No socket/connection is opened between web/api and core in a request trace.

### NFR-WEB-PERF-03: Static assets are cacheable and served locally
- **Requirement**: Vendored static assets (htmx, Alpine, CSS) are served by the binary with
  cache-friendly headers and resolve with no external origin.
- **Test**: Asset requests return 200 with a content type and a cache header; external-origin
  request count on the board = 0 (no-egress host).
- **Linked stories**: US-W02.

---

## NFR-WEB-COMPAT — Backward Compatibility (the regression net)

### NFR-WEB-COMPAT-01: Existing acceptance scenarios stay green
- **Requirement**: Every scenario in `foundry-acceptance` that passes before the extraction
  passes after it. The suite is the binding regression contract.
- **Test**: `cargo test -p foundry-acceptance --release` — the `[Summary]` passing count does
  not drop; no scenario regresses. The browser session/CSRF scenarios stay green even as the
  additive machine-token path is introduced.
- **Linked stories**: US-W05a, US-W05b, US-W05c, US-W01, US-W03, US-W04.
- **Job link**: jtbd-outcome-7 (upgrade/change without breakage); jtbd-web-4 (API is additive).

### NFR-WEB-COMPAT-02: Render contract (asserted substrings) preserved
- **Requirement**: HTML substrings the acceptance suite asserts on — column labels
  ("Backlog", "Todo", "In-Progress", "Done"), issue-key format, `hx-swap-oob` targets,
  `data-*` markers (`data-hx-fragment`, `data-comment-list`, `data-column`,
  `.attachments-empty`), and error copy ("Title is required", "You may only edit your own
  comments.", "This comment has been deleted. Refresh to see the latest state.",
  "Invalid email or password") — render byte-identically from the templates.
- **Test**: The unchanged acceptance scenarios that assert these substrings stay green.
- **Linked stories**: US-W01, US-W02, US-W03, US-W04.

### NFR-WEB-COMPAT-03: CSRF contract unchanged
- **Requirement**: The double-submit pattern is preserved exactly: non-HttpOnly `foundry_csrf`
  cookie set on GET, `_csrf` hidden field on forms, `HX-CSRF`/`hx-csrf` header on htmx
  mutating calls, `/bootstrap` exempt, 403 on missing/invalid token, constant-time compare.
- **Test**: POST without a valid token → 403 (unchanged); cookie/header names unchanged.
- **Linked stories**: US-W01, US-W03, US-W04.
- **Job link**: preserves backend-mvp NFR-SEC-04.

### NFR-WEB-COMPAT-04: Session contract unchanged
- **Requirement**: tower-sessions Postgres store unchanged; session cookie attributes
  (HttpOnly, Secure, SameSite=Lax, 30-day TTL) unchanged; no in-memory session state added.
- **Test**: Inspect Set-Cookie after sign-in (matches backend-mvp NFR-SEC-03).
- **Linked stories**: US-W04.

### NFR-WEB-COMPAT-05: Non-enumerable auth error preserved
- **Requirement**: The sign-in error remains "Invalid email or password" for both unknown
  email and wrong password (no user enumeration), after the template move.
- **Test**: Same string for both cases (unchanged).
- **Linked stories**: US-W04.

---

## NFR-WEB-A11Y — Accessibility & Keyboard (preserved / improved)

### NFR-WEB-A11Y-01: Keyboard operability preserved
- **Requirement**: All interactive controls on the board, issue page, and sign-in are
  keyboard-reachable; the existing `c`-to-create and other shortcuts continue to work; focus
  indicators are visible. WCAG 2.2 AA operable.
- **Test**: Keyboard-only traversal reaches every control; focus indicator present;
  backend-mvp keyboard scenarios (US-12) stay green.
- **Linked stories**: US-W02, US-W03.

### NFR-WEB-A11Y-02: Semantic, contrast-compliant rendering
- **Requirement**: Templates emit valid semantic HTML; form inputs have associated labels;
  text contrast ≥4.5:1 (3:1 large); interactive targets ≥24×24 px. WCAG 2.2 AA.
- **Test**: Automated a11y lint on the board/issue/sign-in templates; contrast check on the
  vendored stylesheet.
- **Linked stories**: US-W02, US-W03, US-W04.

---

## NFR-WEB-MAINT — Maintainability (the contributor payoff)

### NFR-WEB-MAINT-01: Markup lives in templates, not handlers
- **Requirement**: On-screen text and markup for the extracted surfaces live in template
  files under the web tier, not in handler `format!` literals. Grepping for on-screen text
  lands in `templates/`, not in `issues.rs`/`comments.rs`/`signin.rs`.
- **Test**: Code inspection — extracted surfaces have no inline HTML `format!` in handlers.
- **Linked stories**: US-W01, US-W03, US-W04.
- **Job link**: jtbd-web-1.

### NFR-WEB-MAINT-02: One partial per repeated component
- **Requirement**: The issue-card and the comment-card each have ONE template definition,
  consumed by full-page, htmx-fragment, and SSE render paths. No component is rendered by
  more than one definition.
- **Test**: Code inspection — single issue-card partial, single comment-card partial; the
  live-vs-reloaded card structural-equality scenario (US-W03) stays green.
- **Linked stories**: US-W01, US-W03.
- **Job link**: jtbd-web-1.

---

## NFR-WEB-INFRA — Infrastructure invariants (no new services)

### NFR-WEB-INFRA-01: No new runtime services or dependencies
- **Requirement**: The extraction adds NO new runtime service (no Redis, no Node server, no
  separate api process), NO new container, and NO CDN dependency. Still one binary, one
  Postgres, `docker compose up`.
- **Test**: `docker compose` topology unchanged (foundry + postgres); no outbound origin on
  page render.
- **Job link**: jtbd-web-3 + brand promise.

### NFR-WEB-INFRA-02: No build-time secrets; image identical across deployments
- **Requirement**: (inherits backend-mvp posture) Any asset build step is build-time only and
  introduces no runtime secret; the produced image is identical across deployments.
- **Test**: `docker inspect` shows no new runtime secret introduced by the web tier.

---

## NFR-MATRIX — Story-to-NFR Coverage Matrix

Columns: the primary JSON-API track (US-W05a/b/c), the secondary web track (US-W01..W04), and
the boundary guard (US-W06).

| NFR | W05a | W05b | W05c | W06 | W01 | W02 | W03 | W04 |
|-----|------|------|------|-----|-----|-----|-----|-----|
| API-CON-01 | x |   | x |   |   |   |   |   |
| API-CON-02 |   |   | x |   |   |   |   |   |
| API-CON-03 | x |   | x | x |   |   |   |   |
| API-SEC-01 |   | x |   |   |   |   |   |   |
| API-SEC-02 |   | x | x |   |   |   |   |   |
| API-SEC-03 |   | x |   |   |   |   |   |   |
| BND-01    |   |   |   | x | x |   |   |   |
| BND-02    | x |   | x | x |   |   |   |   |
| BND-03    |   |   | x |   |   |   | x |   |
| BND-04    | x | x | x |   | x | x | x | x |
| BND-05    | x |   |   |   | x |   |   |   |
| PERF-01   |   |   |   |   | x | x | x | x |
| PERF-02   | x |   | x |   | x |   | x |   |
| PERF-03   |   |   |   |   |   | x |   |   |
| COMPAT-01 | x | x | x |   | x |   | x | x |
| COMPAT-02 |   |   |   |   | x | x | x | x |
| COMPAT-03 |   | x | x |   | x |   | x | x |
| COMPAT-04 |   | x |   |   |   |   |   | x |
| COMPAT-05 |   |   |   |   |   |   |   | x |
| A11Y-01   |   |   |   |   |   | x | x |   |
| A11Y-02   |   |   |   |   |   | x | x | x |
| MAINT-01  |   |   |   |   | x |   | x | x |
| MAINT-02  |   |   |   |   | x |   | x |   |
| INFRA-01  | x | x | x |   | x | x |   |   |
| INFRA-02  |   |   |   |   |   | x |   |   |
